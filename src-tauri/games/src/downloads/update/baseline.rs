//! The install-local baseline (`.drop-baseline.json`) and the real-disk side
//! of the planner: streaming hashes and a [`DiskView`] over an install folder.
//!
//! The baseline is a JSON sidecar rather than a field in the pot-encoded
//! database or `.dropdata`: adding fields to those has cost real data before,
//! and a sidecar that fails to parse only costs a fallback to the server's
//! file list, never the install.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utils::path_guard;

use super::BASELINE_FILE;
use super::plan::{BaselineFile, DiskStat, DiskView, RemoteFile};

const READ_BUF_LEN: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidecar {
    pub game_id: String,
    pub version_id: String,
    pub revision: u32,
    pub files: Vec<BaselineFile>,
    /// Files the player chose to keep their own copy of in an update,
    /// including files the update removed (those are not in `files`). A
    /// repair keeps a `.bak` of these before restoring the game's copy, and
    /// no update takes them for the pack's copy or removes them as leftovers.
    #[serde(default)]
    pub kept_mine: Vec<String>,
    /// Written by an install, repair or update from a build that heals what
    /// 6.1.0/6.1.1 left behind under the player-data folders (the first
    /// healing pass in the `plan` docs). Once set, updates of this install
    /// no longer run that pass. Absent (false) in sidecars those builds
    /// wrote.
    #[serde(default)]
    pub healed_protected_folders: bool,
    /// As `healed_protected_folders`, for the second healing pass: asking
    /// about files an earlier revision shipped whose original contents are
    /// unknown (`removed_unknown` in the `plan` docs). Absent (false) in
    /// sidecars written before 6.1.3, so every existing install gets that
    /// pass once.
    #[serde(default)]
    pub healed_unknown_leftovers: bool,
}

/// The sidecar without its file list, for the update check (which reads one
/// per install every 30 minutes and only needs the revision).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarHeader {
    pub game_id: String,
    pub version_id: String,
    pub revision: u32,
}

pub fn sidecar_path(install_dir: &Path) -> PathBuf {
    install_dir.join(BASELINE_FILE)
}

/// `Ok(None)` when there is no sidecar; `Err` when one exists and can't be
/// read or parsed (the caller falls back to the server, and says so).
pub fn read_sidecar(install_dir: &Path) -> io::Result<Option<Sidecar>> {
    read_json(&sidecar_path(install_dir))
}

pub fn read_sidecar_header(install_dir: &Path) -> io::Result<Option<SidecarHeader>> {
    read_json(&sidecar_path(install_dir))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> io::Result<Option<T>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    serde_json::from_reader(io::BufReader::new(file))
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write `bytes` to `path` so a crash leaves either the old file or the new
/// one: a temp file next to it, flushed to disk, renamed over.
pub fn write_file_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("no file name"))?
        .to_string_lossy()
        .to_string();
    let tmp = path.with_file_name(format!("{name}.tmp"));
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        // Best effort: the temp file is ours and useless now.
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    sync_dir(path.parent());
    Ok(())
}

/// Flush a directory entry (a rename) to disk. Unix only: Windows has no
/// handle for it and NTFS journals renames itself.
pub fn sync_dir(dir: Option<&Path>) {
    #[cfg(unix)]
    if let Some(dir) = dir
        && let Ok(d) = File::open(dir)
    {
        // A failed directory fsync only weakens power-loss safety; the rename
        // itself already happened.
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

pub fn write_sidecar(dir: &Path, sidecar: &Sidecar) -> io::Result<()> {
    let bytes = serde_json::to_vec(sidecar).map_err(io::Error::other)?;
    write_file_atomic(&sidecar_path(dir), &bytes)
}

/// SHA-256 of a file, streamed (game files run to many gigabytes).
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_BUF_LEN];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Modification time in nanoseconds since the Unix epoch.
pub fn mtime_nanos(meta: &std::fs::Metadata) -> Option<u64> {
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(since.as_nanos()).ok()
}

pub fn stat_path(path: &Path) -> DiskStat {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => DiskStat::File {
            size: m.len(),
            mtime: mtime_nanos(&m),
        },
        Ok(m) if m.is_dir() => DiskStat::Dir,
        Ok(_) => DiskStat::Other,
        Err(e) if e.kind() == io::ErrorKind::NotFound => DiskStat::Missing,
        // A path under a FILE (`data/x` when `data` is a file): nothing is
        // there, and the planner removes the file before putting a folder.
        Err(e) if e.kind() == io::ErrorKind::NotADirectory => DiskStat::Missing,
        // Can't look at it (permissions, a broken parent): not "missing", or
        // the update would write over whatever is there.
        Err(_) => DiskStat::Other,
    }
}

/// The planner's view of a real install folder. Paths are joined the way the
/// downloader joins them (`path_guard::join_within`); an unsafe path reads as
/// `Other`, never as missing.
pub struct FsDisk {
    pub root: PathBuf,
}

impl FsDisk {
    fn path(&self, rel: &str) -> Option<PathBuf> {
        path_guard::join_within(&self.root, Path::new(rel)).ok()
    }
}

impl DiskView for FsDisk {
    fn stat(&self, rel: &str) -> DiskStat {
        match self.path(rel) {
            Some(p) => stat_path(&p),
            None => DiskStat::Other,
        }
    }

    fn sha256(&self, rel: &str) -> io::Result<String> {
        let p = self
            .path(rel)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unsafe path"))?;
        sha256_file(&p)
    }

    fn files_under(&self, rel: &str) -> io::Result<Vec<String>> {
        let p = self
            .path(rel)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unsafe path"))?;
        let mut out = Vec::new();
        let mut stack: Vec<(PathBuf, String)> = vec![(p, rel.replace('\\', "/"))];
        while let Some((dir, prefix)) = stack.pop() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                let child = format!("{prefix}/{name}");
                // symlink_metadata: a link is an entry, never followed.
                if std::fs::symlink_metadata(entry.path())?.is_dir() {
                    stack.push((entry.path(), child));
                } else {
                    out.push(child);
                }
            }
        }
        Ok(out)
    }
}

/// The baseline for a freshly installed (or repaired) folder: the server's
/// list for the revision that was installed, with each file's mtime recorded
/// when its size matches (validation has just checked the content against
/// the manifest). A file whose size differs gets no mtime, so it is hashed
/// before the next update trusts it.
pub fn fresh_sidecar(
    install_dir: &Path,
    game_id: &str,
    version_id: &str,
    revision: u32,
    files: &[RemoteFile],
) -> Sidecar {
    let disk = FsDisk {
        root: install_dir.to_path_buf(),
    };
    let files = files
        .iter()
        .map(|f| {
            let mtime = match disk.stat(&f.path) {
                DiskStat::File { size, mtime } if size == f.size => mtime,
                _ => None,
            };
            let mut entry = BaselineFile::from_remote(f, mtime);
            // The server does not know this file's hash (`""`). The bytes on
            // disk were just verified chunk by chunk, so record their real
            // hash; failing that keep `""` with no mtime, which reads as
            // "unknown" and never as a hash of what is on disk.
            if entry.sha256.is_empty() {
                match (mtime, disk.sha256(&f.path)) {
                    (Some(_), Ok(h)) => entry.sha256 = h,
                    _ => entry.mtime = None,
                }
            }
            entry
        })
        .collect();
    Sidecar {
        game_id: game_id.to_string(),
        version_id: version_id.to_string(),
        revision,
        files,
        kept_mine: Vec::new(),
        // Not known here: a repair or reinstall into a folder an earlier
        // update left files in has not been healed. See `carry_over`.
        healed_protected_folders: false,
        healed_unknown_leftovers: false,
    }
}

/// What a fresh baseline (after an install or repair into `install_dir`)
/// keeps from the sidecar that was already there, read as `previous`:
/// - the healed markers (both passes): a repair only re-checks the files the
///   version ships, and the stale-file sweep never touches the player-data
///   folders, so what 6.1.0/6.1.1 left there survives it. Only a folder with
///   no sidecar (a new install), or one from another game, starts healed. An
///   unreadable sidecar does not.
/// - "keep mine" records for files the version no longer ships that are
///   still on disk (a `removed_edited` file the player kept), so they stay
///   the player's. Records for files the version ships are dropped: the
///   repair has just restored the game's copy (keeping a `.bak` first).
pub fn carry_over(previous: io::Result<Option<Sidecar>>, next: &mut Sidecar, install_dir: &Path) {
    let previous = match previous {
        Ok(None) => {
            next.healed_protected_folders = true;
            next.healed_unknown_leftovers = true;
            return;
        }
        Ok(Some(s)) if s.game_id != next.game_id => {
            next.healed_protected_folders = true;
            next.healed_unknown_leftovers = true;
            return;
        }
        Ok(Some(s)) => s,
        Err(_) => {
            next.healed_protected_folders = false;
            next.healed_unknown_leftovers = false;
            return;
        }
    };
    next.healed_protected_folders = previous.healed_protected_folders;
    next.healed_unknown_leftovers = previous.healed_unknown_leftovers;
    let shipped: std::collections::HashSet<&str> = next.files.iter().map(|f| f.path.as_str()).collect();
    let still_kept: Vec<String> = previous
        .kept_mine
        .into_iter()
        .filter(|p| !shipped.contains(p.as_str()))
        .filter(|p| {
            path_guard::join_within(install_dir, Path::new(p))
                .is_ok_and(|full| std::fs::symlink_metadata(full).is_ok_and(|m| m.is_file()))
        })
        .collect();
    for p in still_kept {
        if !next.kept_mine.contains(&p) {
            next.kept_mine.push(p);
        }
    }
    next.kept_mine.sort();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-baseline-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sha256_matches_a_known_digest_across_buffer_boundaries() {
        let dir = scratch("hash");
        let p = dir.join("f");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Larger than one read buffer.
        let big = vec![7u8; READ_BUF_LEN * 2 + 3];
        std::fs::write(&p, &big).unwrap();
        assert_eq!(sha256_file(&p).unwrap(), hex::encode(Sha256::digest(&big)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_round_trips_and_a_missing_one_is_none() {
        let dir = scratch("sidecar");
        assert!(read_sidecar(&dir).unwrap().is_none());
        let s = Sidecar {
            game_id: "g".into(),
            version_id: "v".into(),
            revision: 3,
            files: vec![BaselineFile {
                path: "a/b.jar".into(),
                size: 1,
                sha256: "00".into(),
                mtime: Some(u64::MAX - 1),
            }],
            kept_mine: vec!["a/b.jar".into()],
            healed_protected_folders: true,
            healed_unknown_leftovers: true,
        };
        write_sidecar(&dir, &s).unwrap();
        assert_eq!(read_sidecar(&dir).unwrap(), Some(s));
        let header = read_sidecar_header(&dir).unwrap().unwrap();
        assert_eq!(header.revision, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sidecar_from_6_1_loads_as_not_yet_healed() {
        let dir = scratch("sidecar-61");
        // As 6.1.1 wrote it: no healedProtectedFolders.
        std::fs::write(
            sidecar_path(&dir),
            br#"{"gameId":"g","versionId":"v","revision":2,"files":[{"path":"a","size":1,"sha256":"00","mtime":5}],"keptMine":[]}"#,
        )
        .unwrap();
        let s = read_sidecar(&dir).unwrap().unwrap();
        assert!(!s.healed_protected_folders && !s.healed_unknown_leftovers);
        assert_eq!(s.files.len(), 1);
        let json = serde_json::to_string(&Sidecar {
            healed_protected_folders: true,
            ..s
        })
        .unwrap();
        assert!(json.contains(r#""healedProtectedFolders":true"#), "{json}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sidecar_from_6_1_2_loads_with_the_second_pass_due() {
        let dir = scratch("sidecar-612");
        // As 6.1.2 wrote it: first pass done, no healedUnknownLeftovers.
        std::fs::write(
            sidecar_path(&dir),
            br#"{"gameId":"g","versionId":"v","revision":2,"files":[],"keptMine":[],"healedProtectedFolders":true}"#,
        )
        .unwrap();
        let s = read_sidecar(&dir).unwrap().unwrap();
        assert!(s.healed_protected_folders && !s.healed_unknown_leftovers);
        let json = serde_json::to_string(&Sidecar {
            healed_unknown_leftovers: true,
            ..s
        })
        .unwrap();
        assert!(json.contains(r#""healedUnknownLeftovers":true"#), "{json}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_new_install_starts_healed_and_a_repair_keeps_the_marker() {
        let dir = scratch("carry-over");
        std::fs::create_dir_all(dir.join("user/mods")).unwrap();
        std::fs::write(dir.join("user/mods/kept-removed.jar"), b"player").unwrap();
        let files = [RemoteFile {
            path: "user/mods/a.jar".into(),
            size: 1,
            sha256: "00".into(),
        }];
        let fresh = || fresh_sidecar(&dir, "g", "v", 2, &files);
        // Not known before `carry_over` has looked.
        assert!(!fresh().healed_protected_folders && !fresh().healed_unknown_leftovers);
        // A new install: nothing an earlier update could have left.
        let mut next = fresh();
        carry_over(Ok(None), &mut next, &dir);
        assert!(next.healed_protected_folders && next.healed_unknown_leftovers);
        // A repair of an install 6.1.x updated: the leftovers survive a
        // repair, so it is still not healed, and the removed file the player
        // kept stays theirs. A record for a shipped file is dropped.
        let before = Sidecar {
            game_id: "g".into(),
            version_id: "v".into(),
            revision: 2,
            files: vec![],
            kept_mine: vec![
                "user/mods/a.jar".into(),
                "user/mods/kept-removed.jar".into(),
                "user/mods/deleted-since.jar".into(),
            ],
            healed_protected_folders: false,
            healed_unknown_leftovers: false,
        };
        let mut next = fresh();
        carry_over(Ok(Some(before.clone())), &mut next, &dir);
        assert!(!next.healed_protected_folders && !next.healed_unknown_leftovers);
        assert_eq!(next.kept_mine, vec!["user/mods/kept-removed.jar".to_string()]);
        // Healed before: stays healed, each marker on its own.
        for (protected, unknown) in [(true, false), (false, true), (true, true)] {
            let mut next = fresh();
            carry_over(
                Ok(Some(Sidecar {
                    healed_protected_folders: protected,
                    healed_unknown_leftovers: unknown,
                    ..before.clone()
                })),
                &mut next,
                &dir,
            );
            assert_eq!(
                (next.healed_protected_folders, next.healed_unknown_leftovers),
                (protected, unknown)
            );
        }
        // Unreadable: not claimed healed. Another game's: a new install.
        let mut next = fresh();
        carry_over(Err(io::Error::other("corrupt")), &mut next, &dir);
        assert!(!next.healed_protected_folders && !next.healed_unknown_leftovers);
        let mut next = fresh();
        carry_over(Ok(Some(Sidecar { game_id: "other".into(), ..before })), &mut next, &dir);
        assert!(next.healed_protected_folders && next.healed_unknown_leftovers && next.kept_mine.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_sidecar_is_an_error_not_an_empty_baseline() {
        let dir = scratch("corrupt");
        std::fs::write(sidecar_path(&dir), b"{not json").unwrap();
        assert!(read_sidecar(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_sidecar_records_mtimes_only_for_files_of_the_right_size() {
        let dir = scratch("fresh");
        std::fs::write(dir.join("ok.bin"), b"1234").unwrap();
        std::fs::write(dir.join("short.bin"), b"12").unwrap();
        let files = [
            RemoteFile {
                path: "ok.bin".into(),
                size: 4,
                sha256: "x".into(),
            },
            RemoteFile {
                path: "short.bin".into(),
                size: 4,
                sha256: "y".into(),
            },
            RemoteFile {
                path: "missing.bin".into(),
                size: 1,
                sha256: "z".into(),
            },
        ];
        let s = fresh_sidecar(&dir, "g", "v", 1, &files);
        assert!(s.files[0].mtime.is_some());
        assert!(s.files[1].mtime.is_none());
        assert!(s.files[2].mtime.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fs_disk_never_reports_an_unsafe_path_as_missing() {
        let dir = scratch("unsafe");
        let disk = FsDisk { root: dir.clone() };
        assert_eq!(disk.stat("../x"), DiskStat::Other);
        assert_eq!(disk.stat("nope"), DiskStat::Missing);
        std::fs::create_dir(dir.join("d")).unwrap();
        assert_eq!(disk.stat("d"), DiskStat::Dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fresh_install_records_the_real_hash_for_an_unknown_one() {
        let dir = scratch("unknown");
        std::fs::write(dir.join("a.bin"), b"abc").unwrap();
        let files = [
            RemoteFile {
                path: "a.bin".into(),
                size: 3,
                sha256: String::new(),
            },
            RemoteFile {
                path: "gone.bin".into(),
                size: 3,
                sha256: String::new(),
            },
        ];
        let s = fresh_sidecar(&dir, "g", "v", 1, &files);
        assert_eq!(s.files[0].sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(s.files[0].mtime.is_some());
        // Can't be read: stays unknown, and never carries an mtime.
        assert_eq!(s.files[1].sha256, "");
        assert!(s.files[1].mtime.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }


    #[test]
    fn a_path_under_a_file_reads_as_missing_and_folders_are_listed() {
        let dir = scratch("enotdir");
        std::fs::write(dir.join("data"), b"file").unwrap();
        let disk = FsDisk { root: dir.clone() };
        assert_eq!(disk.stat("data/x"), DiskStat::Missing);
        std::fs::create_dir_all(dir.join("f/sub")).unwrap();
        std::fs::write(dir.join("f/a"), b"1").unwrap();
        std::fs::write(dir.join("f/sub/b"), b"2").unwrap();
        assert_eq!(disk.stat("f"), DiskStat::Dir);
        let mut under = disk.files_under("f").unwrap();
        under.sort();
        assert_eq!(under, vec!["f/a".to_string(), "f/sub/b".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
