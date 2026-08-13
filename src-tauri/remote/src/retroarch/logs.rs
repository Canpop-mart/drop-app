//! Reading RetroArch's own log after a session.
//!
//! Drop turns on `log_to_file` + `log_to_file_timestamp` for every RetroArch
//! launch, which gives one `logs/retroarch__*.log` per session. That file is
//! the only place RetroArch states two things Drop otherwise cannot see:
//!
//! * RetroAchievements refused the Connect token (see [`super::ra`]),
//! * the video driver failed to initialise, which kills RetroArch during
//!   startup with no window ever appearing — the game just doesn't show up.
//!
//! Both are read on the exit path, so this module owns the shared "find the
//! log this session wrote, scan it for a marker" plumbing.

use log::{debug, info};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How much of a RetroArch log to read before giving up. Drop turns on
/// verbose file logging, so a long session's log can reach hundreds of MB —
/// every marker here is written during startup, well inside this bound.
pub(super) const MAX_LOG_SCAN_BYTES: u64 = 4 * 1024 * 1024;

/// Substrings RetroArch logs when it cannot bring up video and exits.
///
/// The first is the frontend giving up (it is followed by
/// `Fatal error received in: "video_driver_init_internal()"` and an exit); the
/// other two are the driver-specific failures that lead to it. Any one is
/// enough — the driver-specific line is kept when present because it names
/// which backend died.
const FATAL_VIDEO_MARKERS: &[&str] = &[
    "Cannot open video driver",
    "Failed to set video mode",
    "video_driver_init_internal()",
];

/// Most recently modified `retroarch__*.log` under `emu_root/logs/` that was
/// touched at or after `launched_at`.
///
/// `log_to_file_timestamp` gives one file per launch, so the newest file is
/// the session that just ended — but only if that session wrote one at all.
/// Drop's `log_dir` only reaches RetroArch when `--appendconfig` was injected
/// (or the AppImage-home copy applied), so a script-wrapped plain install
/// leaves the directory untouched. Rejecting everything older than the launch
/// makes that case yield `None` instead of re-reading some earlier session's
/// contents and attributing it to this one.
pub(super) fn newest_retroarch_log(emu_root: &Path, launched_at: SystemTime) -> Option<PathBuf> {
    let logs_dir = emu_root.join("logs");
    let entries = match std::fs::read_dir(&logs_dir) {
        Ok(e) => e,
        Err(e) => {
            debug!("[RA-LOG] No RetroArch log dir at {}: {e}", logs_dir.display());
            return None;
        }
    };

    let newest = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.starts_with("retroarch__") && name.ends_with(".log")
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            (modified >= launched_at).then(|| (modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path);

    if newest.is_none() {
        debug!(
            "[RA-LOG] No RetroArch log in {} written by this session — \
             not reading older logs",
            logs_dir.display()
        );
    }
    newest
}

/// Returns the first line of `log_path` containing any of `markers`, trimmed.
///
/// Scanned as bytes, not `lines()`: RetroArch writes the content path during
/// load, so a ROM filename in Shift-JIS or Latin-1 would abort a UTF-8 line
/// iterator before it ever reached the answer. Lossy-decoding each line keeps
/// the markers (pure ASCII) findable whatever the rest of the line holds.
pub(super) fn first_line_matching(log_path: &Path, markers: &[&str]) -> Option<String> {
    let file = match std::fs::File::open(log_path) {
        Ok(f) => f,
        Err(e) => {
            debug!("[RA-LOG] Could not open {}: {e}", log_path.display());
            return None;
        }
    };

    let mut reader = BufReader::new(file.take(MAX_LOG_SCAN_BYTES));
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match reader.read_until(b'\n', &mut raw) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(e) => {
                debug!("[RA-LOG] Read error scanning {}: {e}", log_path.display());
                return None;
            }
        }
        let line = String::from_utf8_lossy(&raw);
        if markers.iter().any(|m| line.contains(m)) {
            return Some(line.trim().to_string());
        }
    }
}

/// Scans this session's RetroArch log for a fatal video-driver failure.
///
/// RetroArch treats "cannot open video driver" as fatal: it logs it and exits
/// during startup, before any window exists. From outside, that is
/// indistinguishable from the Play button doing nothing, which is how the
/// Steam Deck's Castlevania launch presented — RetroArch exited immediately
/// and Drop reported nothing at all.
///
/// Returns the offending log line so the caller can show the user what
/// RetroArch actually said rather than a paraphrase.
pub fn detect_fatal_video_error(emu_root: &Path, launched_at: SystemTime) -> Option<String> {
    let log_path = newest_retroarch_log(emu_root, launched_at)?;
    let line = first_line_matching(&log_path, FATAL_VIDEO_MARKERS)?;
    info!(
        "[RA-LOG] RetroArch failed to start its video driver ({}): {line}",
        log_path.display()
    );
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::{first_line_matching, FATAL_VIDEO_MARKERS};
    use std::io::Write;

    fn write_log(name: &str, body: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&path).expect("temp log");
        f.write_all(body.as_bytes()).expect("write temp log");
        path
    }

    /// The real Castlevania/swanstation tail from the Deck.
    #[test]
    fn finds_the_vulkan_video_mode_failure() {
        let path = write_log(
            "drop-ra-video-fail.log",
            "[INFO] [Video]: Found display server: wayland\n\
             [ERROR] [Vulkan]: Failed to set video mode.\n\
             [ERROR] [Video]: Cannot open video driver. Exiting...\n\
             [ERROR] Fatal error received in: \"video_driver_init_internal()\"\n",
        );
        assert_eq!(
            first_line_matching(&path, FATAL_VIDEO_MARKERS).as_deref(),
            Some("[ERROR] [Vulkan]: Failed to set video mode.")
        );
        let _ = std::fs::remove_file(path);
    }

    /// A healthy session must not be reported as a video failure.
    #[test]
    fn a_clean_log_matches_nothing() {
        let path = write_log(
            "drop-ra-video-ok.log",
            "[INFO] [Video]: Video @ 1280x800\n[INFO] [D3D11]: Init complete.\n",
        );
        assert_eq!(first_line_matching(&path, FATAL_VIDEO_MARKERS), None);
        let _ = std::fs::remove_file(path);
    }

    /// A ROM name that is not valid UTF-8 must not stop the scan short.
    #[test]
    fn invalid_utf8_earlier_in_the_log_does_not_hide_the_error() {
        let path = std::env::temp_dir().join("drop-ra-video-latin1.log");
        let mut body: Vec<u8> = b"[INFO] Loading content: Pok\xe9mon.iso\n".to_vec();
        body.extend_from_slice(b"[ERROR] [Video]: Cannot open video driver. Exiting...\n");
        std::fs::write(&path, &body).expect("write temp log");
        assert!(
            first_line_matching(&path, FATAL_VIDEO_MARKERS)
                .is_some_and(|l| l.contains("Cannot open video driver"))
        );
        let _ = std::fs::remove_file(path);
    }
}
