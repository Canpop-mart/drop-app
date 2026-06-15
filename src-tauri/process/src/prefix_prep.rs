//! One-time, idempotent Proton-prefix preparation for umu/Proton launches.
//!
//! A freshly-created Proton prefix is bare. Two distinct categories of Windows
//! game need a prefix touched *before* the first launch or they fail in ways
//! that look like Drop bugs but are actually missing runtime plumbing. Both
//! recipes here were reverse-engineered and confirmed working by hand on a
//! Steam Deck; this module bakes the proven steps into code so users never
//! have to run winetricks or shuffle DLLs themselves.
//!
//! Everything is Linux-only — Windows/macOS keep a no-op stub so the workspace
//! still compiles with `#![deny]` lints and zero unused warnings. The public
//! entry point is [`prepare_prefix`]; it performs side effects (installing the
//! VC++ runtime, copying DLLs, creating symlinks) and returns the extra
//! `(KEY, VALUE)` env pairs the caller must append to the umu command. It
//! NEVER fails the launch: anything that goes wrong is logged at `warn` and we
//! return whatever env we managed to assemble.
//!
//! ## Branch A — VC++ runtime (the "Far Far West" category)
//! Many Windows games link the MSVC 2015-2022 runtime (`VCRUNTIME140.dll`,
//! `MSVCP140.dll`). A fresh Proton prefix has none of it, and running the raw
//! `VC_redist.exe` does nothing under wine — its MSI is blocked by wine's
//! higher builtin `ucrtbase`. winetricks (which umu runs natively) is the fix.
//!
//! ## Branch B — OnlineFix (online co-op cracks, the "Gunfire Reborn" category)
//! OnlineFix payloads ship an `OnlineFix64.dll` next to the game exe and expect
//! a live Steam install's overlay DLL plus Proton's Steam bridge present at
//! hardcoded Windows paths inside the prefix. We stage those DLLs, ensure the
//! Linux-side `steamclient.so` symlinks exist, and hand back the canonical
//! DLL-override env set.

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

/// Marker file (relative to the prefix root) recording that Branch A has
/// already provisioned the VC++ runtime into this prefix. Versioned so a future
/// recipe change can force a re-provision by bumping the suffix.
#[cfg(target_os = "linux")]
const REDIST_MARKER: &str = ".drop-redists-v1";

/// Prepare a Proton prefix for a umu launch, running both independent branches.
///
/// * `game_id`     — the game's id, used only to scope the `game_prep_status`
///   UI events so the right game-detail page picks them up.
/// * `install_dir` — the game's install directory (working dir of the launch).
/// * `exe_path`    — the absolute path to the launch executable. Used to sniff
///   the PE import table (Branch A) and to locate the OnlineFix payload
///   directory (Branch B).
/// * `pfx_dir`     — the Proton prefix root (`WINEPREFIX`).
/// * `proton_path` — the resolved `PROTONPATH`: either an absolute Proton
///   install dir or a umu keyword like `GE-Proton` that umu auto-resolves.
/// * `umu_exe`     — the `umu-run` executable Branch A invokes for winetricks.
///
/// Returns the extra `(KEY, VALUE)` env pairs to add to the umu command. The
/// pairs come solely from Branch B; Branch A is pure side effect. On non-Linux
/// targets, or when nothing applies, the returned `Vec` is empty.
#[cfg(target_os = "linux")]
pub fn prepare_prefix(
    game_id: &str,
    install_dir: &str,
    exe_path: &str,
    pfx_dir: &Path,
    proton_path: &str,
    umu_exe: &str,
) -> Vec<(String, String)> {
    log::info!(
        "[PrefixPrep] Preparing prefix {:?} for exe {:?} (install_dir={:?})",
        pfx_dir,
        exe_path,
        install_dir
    );

    // Branch A: VC++ runtime. Pure side effect, returns no env.
    prepare_vcredist(game_id, exe_path, pfx_dir, proton_path, umu_exe);

    // Branch B: OnlineFix. Side effects + the override env the caller appends.
    let env = prepare_onlinefix(exe_path, pfx_dir, proton_path);

    log::info!(
        "[PrefixPrep] Prefix preparation finished; returning {} extra env var(s)",
        env.len()
    );
    env
}

/// Non-Linux stub. Windows/macOS prefixes don't exist; references its args so
/// the unused-variable lint stays quiet under `#![deny]`.
#[cfg(not(target_os = "linux"))]
pub fn prepare_prefix(
    game_id: &str,
    install_dir: &str,
    exe_path: &str,
    pfx_dir: &std::path::Path,
    proton_path: &str,
    umu_exe: &str,
) -> Vec<(String, String)> {
    let _ = (game_id, install_dir, exe_path, pfx_dir, proton_path, umu_exe);
    Vec::new()
}

/// Emit a `game_prep_status` Tauri event so the game-detail UI can surface a
/// "Preparing…" status while a slow, blocking prefix-prep step runs (the first
/// `launch_game` invoke doesn't return until prep finishes, so without this the
/// window looks frozen). `message: Some(_)` means "prep active, show this text";
/// `None` means "prep done, clear the indicator". No-op if the launch
/// AppHandle hasn't been set yet (e.g. unit tests).
#[cfg(target_os = "linux")]
fn emit_prep_status(game_id: &str, message: Option<&str>) {
    use tauri::Emitter;

    if let Some(app_handle) = crate::LAUNCH_APP_HANDLE.get() {
        let payload = serde_json::json!({
            "gameId": game_id,
            "active": message.is_some(),
            "message": message.unwrap_or(""),
        });
        if let Err(e) = app_handle.emit("game_prep_status", payload) {
            log::warn!("[PrefixPrep] Failed to emit game_prep_status event: {}", e);
        }
    }
}

// ───────────────────────────── Branch A ─────────────────────────────────────

/// Branch A: provision the MSVC 2015-2022 runtime into the prefix if (and only
/// if) the launch exe imports it, and only once per prefix.
///
/// Detection is a deliberately crude, dependency-free heuristic: we read up to
/// the first 64 MiB of the exe and case-insensitively search for the ASCII
/// needles `VCRUNTIME140` / `MSVCP140`, which appear verbatim as DLL-name
/// strings in a PE's import table. No PE crate, no parsing — a false positive
/// merely runs an idempotent winetricks install that no-ops, and a false
/// negative just skips a runtime the game probably didn't need.
///
/// Idempotency is enforced by the [`REDIST_MARKER`] file: present ⇒ skip. The
/// marker is written ONLY after winetricks exits successfully, so a failed or
/// interrupted install retries next launch. winetricks itself is also
/// idempotent ("already installed"), so a redundant run is harmless.
#[cfg(target_os = "linux")]
fn prepare_vcredist(
    game_id: &str,
    exe_path: &str,
    pfx_dir: &Path,
    proton_path: &str,
    umu_exe: &str,
) {
    let marker = pfx_dir.join(REDIST_MARKER);
    if marker.exists() {
        log::info!("[PrefixPrep/A] VC++ runtime marker present — skipping provision");
        return;
    }

    if !exe_imports_msvc_runtime(exe_path) {
        // Record the negative result so we don't re-read the exe on every
        // launch. The marker means "redists resolved for this prefix" — here,
        // resolved as "not needed". Bump REDIST_MARKER to force a re-check.
        log::info!(
            "[PrefixPrep/A] {:?} does not import the MSVC runtime — recording check, skipping VC++ provision",
            exe_path
        );
        if let Err(e) = std::fs::write(&marker, b"not-needed\n") {
            log::warn!(
                "[PrefixPrep/A] Could not write redist marker {:?}: {}",
                marker,
                e
            );
        }
        return;
    }

    log::info!(
        "[PrefixPrep/A] {:?} imports the MSVC runtime — installing vcrun2022 + d3dcompiler_47 \
         via winetricks (umu). First run downloads from aka.ms and needs network.",
        exe_path
    );

    // Run winetricks as its own child process — NOT appended to the launch
    // string. umu runs winetricks natively when handed it as the command:
    //   GAMEID=0 STORE=none PROTONPATH=<proton> WINEPREFIX=<pfx> \
    //       <umu-run> winetricks -q vcrun2022 d3dcompiler_47
    let mut command = std::process::Command::new(umu_exe);
    command
        .arg("winetricks")
        .arg("-q")
        .arg("vcrun2022")
        .arg("d3dcompiler_47")
        .env("GAMEID", "0")
        .env("STORE", "none")
        .env("PROTONPATH", proton_path)
        .env("WINEPREFIX", pfx_dir)
        // RUST_LOG must not leak into the child; Steam/Gamescope bundled-Python
        // vars break umu's system Python (see env::apply_baseline_env_scrub).
        .env_remove("RUST_LOG")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");

    log::info!("[PrefixPrep/A] Spawning winetricks (this can take a while on first run)…");
    // Surface a precise "installing runtime" status to the UI for the duration
    // of the blocking winetricks run, then clear it on every outcome below.
    emit_prep_status(
        game_id,
        Some("Installing runtime libraries (one-time setup)..."),
    );
    let status_result = command.status();
    emit_prep_status(game_id, None);
    match status_result {
        Ok(status) if status.success() => {
            log::info!("[PrefixPrep/A] winetricks finished successfully — writing marker");
            if let Err(e) = std::fs::write(&marker, b"vcrun2022 d3dcompiler_47\n") {
                log::warn!(
                    "[PrefixPrep/A] Could not write redist marker {:?}: {} \
                     (runtime is installed but we'll re-run winetricks next launch)",
                    marker,
                    e
                );
            }
        }
        Ok(status) => {
            log::warn!(
                "[PrefixPrep/A] winetricks exited with {} — NOT writing marker; \
                 will retry next launch. The game may still launch without the runtime.",
                status
            );
        }
        Err(e) => {
            log::warn!(
                "[PrefixPrep/A] Failed to spawn winetricks via umu ({:?}): {} — \
                 continuing launch without the VC++ runtime.",
                umu_exe,
                e
            );
        }
    }
}

/// Cheap, PE-crate-free check: does `exe_path` reference the MSVC 2015-2022
/// runtime DLLs by name? Reads at most the first 64 MiB and searches the raw
/// bytes (case-insensitively) for `VCRUNTIME140` or `MSVCP140`, the DLL-name
/// strings a PE carries in its import table. Any read error ⇒ `false` (we'd
/// rather skip provisioning than block a launch).
#[cfg(target_os = "linux")]
fn exe_imports_msvc_runtime(exe_path: &str) -> bool {
    use std::io::Read;

    /// Cap the read so a giant single-file exe can't balloon memory.
    const MAX_SCAN_BYTES: u64 = 64 * 1024 * 1024;

    let file = match std::fs::File::open(exe_path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!(
                "[PrefixPrep/A] Could not open {:?} for MSVC-runtime sniff: {} — assuming no runtime needed",
                exe_path,
                e
            );
            return false;
        }
    };

    let mut buf = Vec::new();
    if let Err(e) = file.take(MAX_SCAN_BYTES).read_to_end(&mut buf) {
        log::warn!(
            "[PrefixPrep/A] Could not read {:?} for MSVC-runtime sniff: {} — assuming no runtime needed",
            exe_path,
            e
        );
        return false;
    }

    bytes_contain_ascii_ci(&buf, b"VCRUNTIME140") || bytes_contain_ascii_ci(&buf, b"MSVCP140")
}

/// Case-insensitive ASCII substring search over raw bytes. `needle` must be
/// pure ASCII (our literals are). Sliding window — fine for one-off launch-time
/// scans of bounded buffers; avoids pulling in a regex/aho-corasick dependency.
#[cfg(target_os = "linux")]
fn bytes_contain_ascii_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return needle.is_empty();
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

// ───────────────────────────── Branch B ─────────────────────────────────────

/// Branch B: stage everything an OnlineFix payload needs and return the
/// canonical DLL-override env set.
///
/// Detection: the launch exe's OWN directory contains `OnlineFix64.dll`. If it
/// doesn't, this is not an OnlineFix game and we return an empty env. When it
/// is, we (idempotently, where file ops are involved):
///   1. copy `GameOverlayRenderer64.dll` (from the live Steam install) and
///      Proton's `lsteamclient.dll` (renamed `steamclient64.dll`) into
///      `<pfx>/drive_c/Program Files (x86)/Steam/`;
///   2. ensure `~/.steam/sdk{64,32}/steamclient.so` symlinks exist (Proton's
///      lsteamclient dlopen()s those HARDCODED paths, ignoring env on Linux);
///   3. return `WINEDLLOVERRIDES` + `STEAM_COMPAT_CLIENT_INSTALL_PATH`.
///
/// We deliberately do NOT set `SteamAppId=480`: the game's bundled
/// `steam_appid.txt` already presents Spacewar(480), and forcing it here would
/// break Goldberg achievements (which key off the real appid).
#[cfg(target_os = "linux")]
fn prepare_onlinefix(exe_path: &str, pfx_dir: &Path, proton_path: &str) -> Vec<(String, String)> {
    let exe_dir = match Path::new(exe_path).parent() {
        Some(d) => d.to_path_buf(),
        None => {
            log::warn!(
                "[PrefixPrep/B] Could not determine parent dir of exe {:?} — skipping OnlineFix prep",
                exe_path
            );
            return Vec::new();
        }
    };

    let onlinefix_dll = exe_dir.join("OnlineFix64.dll");
    if !onlinefix_dll.exists() {
        log::info!("[PrefixPrep/B] No OnlineFix64.dll next to the exe — not an OnlineFix game");
        return Vec::new();
    }

    log::info!(
        "[PrefixPrep/B] OnlineFix payload detected ({:?}) — staging Steam DLLs + symlinks",
        onlinefix_dll
    );

    // ── Step 1: stage the two DLLs into the prefix's Steam dir ───────────────
    let steam_dir_in_pfx = pfx_dir
        .join("drive_c")
        .join("Program Files (x86)")
        .join("Steam");
    if let Err(e) = std::fs::create_dir_all(&steam_dir_in_pfx) {
        log::warn!(
            "[PrefixPrep/B] Could not create in-prefix Steam dir {:?}: {} — \
             OnlineFix DLL staging may be incomplete",
            steam_dir_in_pfx,
            e
        );
    }

    // GameOverlayRenderer64.dll — OnlineFix's SteamOverlay64.dll LoadLibrary's
    // this in its DllMain; absent ⇒ "failed to load steam overlay dll error 126".
    match find_live_steam_overlay_dll() {
        Some(src) => copy_if_absent(&src, &steam_dir_in_pfx.join("GameOverlayRenderer64.dll")),
        None => log::warn!(
            "[PrefixPrep/B] Could not find GameOverlayRenderer64.dll in any Steam install — \
             OnlineFix overlay may fail with error 126"
        ),
    }

    // steamclient64.dll — Proton's lsteamclient.dll is the Windows→Linux Steam
    // bridge; OnlineFix looks for C:\Program Files (x86)\Steam\steamclient64.dll
    // which Proton doesn't populate under umu, so we copy it in under that name.
    match find_proton_lsteamclient(proton_path) {
        Some(src) => copy_if_absent(&src, &steam_dir_in_pfx.join("steamclient64.dll")),
        None => log::warn!(
            "[PrefixPrep/B] Could not find Proton's lsteamclient.dll (proton_path={:?}) — \
             OnlineFix Steam bridge will be missing",
            proton_path
        ),
    }

    // ── Step 2: ensure the Linux-side steamclient.so symlinks exist ──────────
    ensure_steam_sdk_symlinks();

    // ── Step 4 (best-effort): warn if Steam isn't running ────────────────────
    if !is_steam_running() {
        log::warn!(
            "[PrefixPrep/B] No running 'steam' process detected — OnlineFix online play \
             needs Steam running. Launch continues regardless."
        );
    }

    // ── Step 3: the env the caller appends to the umu command ────────────────
    let home = home_dir_string();
    let steam_compat_path = format!("{home}/.steam/steam");
    let env = vec![
        (
            "WINEDLLOVERRIDES".to_string(),
            "OnlineFix64=n;SteamOverlay64=n;winmm=n,b;dnet=n;steam_api64=n".to_string(),
        ),
        (
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            steam_compat_path,
        ),
    ];
    log::info!(
        "[PrefixPrep/B] OnlineFix prep complete — returning {} override env var(s)",
        env.len()
    );
    env
}

/// Locate a live Steam install's `GameOverlayRenderer64.dll`. Searches the
/// standard Steam roots (`~/.steam/steam`, `~/.local/share/Steam`,
/// `~/.steam/root`) recursively for the first file by that name. Returns the
/// first hit (any working overlay DLL is interchangeable for OnlineFix's needs).
#[cfg(target_os = "linux")]
fn find_live_steam_overlay_dll() -> Option<PathBuf> {
    for root in steam_install_roots() {
        if let Some(found) = find_file_named(&root, "GameOverlayRenderer64.dll", 6) {
            log::info!("[PrefixPrep/B] Found GameOverlayRenderer64.dll at {:?}", found);
            return Some(found);
        }
    }
    None
}

/// Locate Proton's `lsteamclient.dll` (the Windows→Linux Steam bridge).
///
/// * If `proton_path` is an absolute directory, look directly at the known
///   relative location `<proton>/files/lib/wine/x86_64-windows/lsteamclient.dll`.
/// * If `proton_path` is a umu keyword (`GE-Proton`, `UMU-Proton`, …) that umu
///   auto-resolves, we can't know the install dir, so we search the standard
///   compatibility-tool / umu roots and pick the NEWEST match (most recently
///   installed Proton wins).
#[cfg(target_os = "linux")]
fn find_proton_lsteamclient(proton_path: &str) -> Option<PathBuf> {
    const REL: &str = "files/lib/wine/x86_64-windows/lsteamclient.dll";

    let proton = Path::new(proton_path);
    if proton.is_absolute() && proton.is_dir() {
        let direct = proton.join(REL);
        if direct.is_file() {
            log::info!(
                "[PrefixPrep/B] Found lsteamclient.dll via absolute PROTONPATH: {:?}",
                direct
            );
            return Some(direct);
        }
        log::warn!(
            "[PrefixPrep/B] Absolute PROTONPATH {:?} has no {} — falling back to search",
            proton_path,
            REL
        );
    }

    // Keyword (or a path that didn't resolve): search the standard install
    // roots and keep the newest lsteamclient.dll.
    let home = home_dir_string();
    let search_roots = [
        format!("{home}/.local/share/Steam/compatibilitytools.d"),
        format!("{home}/.steam/root/compatibilitytools.d"),
        format!("{home}/.local/share/umu"),
    ];

    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    for root in &search_roots {
        for hit in find_files_named(Path::new(root), "lsteamclient.dll", 8) {
            let mtime = hit
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            let replace = match &newest {
                Some((_, best)) => mtime > *best,
                None => true,
            };
            if replace {
                newest = Some((hit, mtime));
            }
        }
    }

    match newest {
        Some((path, _)) => {
            log::info!(
                "[PrefixPrep/B] Found newest lsteamclient.dll via keyword search: {:?}",
                path
            );
            Some(path)
        }
        None => None,
    }
}

/// Ensure Proton's HARDCODED Steam-bridge socket paths exist.
///
/// Proton's lsteamclient `dlopen()`s `$HOME/.steam/sdk64/steamclient.so` (and
/// the 32-bit equivalent) by an absolute path that IGNORES env vars on Linux.
/// If those symlinks are missing we create them, pointing at the live Steam
/// install's `linux64`/`linux32` `steamclient.so`. Each side is independent and
/// best-effort: a missing source `.so` is skipped silently (logged at info).
#[cfg(target_os = "linux")]
fn ensure_steam_sdk_symlinks() {
    let home = home_dir_string();
    let steam_root = find_steam_root_with_runtime();
    let Some(steam_root) = steam_root else {
        log::info!(
            "[PrefixPrep/B] No Steam install with linux64/steamclient.so found — \
             skipping sdk symlink setup"
        );
        return;
    };

    let pairs = [
        ("sdk64", "linux64"),
        ("sdk32", "linux32"),
    ];
    for (sdk, linux) in pairs {
        let link = PathBuf::from(format!("{home}/.steam/{sdk}/steamclient.so"));
        let target = steam_root.join(linux).join("steamclient.so");

        if link.exists() {
            // Already present (regular file or a good symlink) — leave it.
            continue;
        }
        if !target.is_file() {
            log::info!(
                "[PrefixPrep/B] Steam bridge source {:?} not found — skipping {} symlink",
                target,
                sdk
            );
            continue;
        }
        if let Some(parent) = link.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            log::warn!(
                "[PrefixPrep/B] Could not create {:?} for steamclient.so symlink: {}",
                parent,
                e
            );
            continue;
        }
        match std::os::unix::fs::symlink(&target, &link) {
            Ok(()) => log::info!(
                "[PrefixPrep/B] Linked {:?} → {:?} (Proton Steam bridge)",
                link,
                target
            ),
            Err(e) => log::warn!(
                "[PrefixPrep/B] Could not symlink {:?} → {:?}: {}",
                link,
                target,
                e
            ),
        }
    }
}

/// The standard Steam install roots to probe, in priority order. Returned as
/// owned `PathBuf`s; non-existent ones are harmless (the callers test for the
/// files they want).
#[cfg(target_os = "linux")]
fn steam_install_roots() -> Vec<PathBuf> {
    let home = home_dir_string();
    vec![
        PathBuf::from(format!("{home}/.steam/steam")),
        PathBuf::from(format!("{home}/.local/share/Steam")),
        PathBuf::from(format!("{home}/.steam/root")),
    ]
}

/// Find the first Steam root that actually contains `linux64/steamclient.so`
/// (i.e. a usable runtime, not just a stale symlink farm). Used to anchor the
/// sdk64/sdk32 symlinks.
#[cfg(target_os = "linux")]
fn find_steam_root_with_runtime() -> Option<PathBuf> {
    steam_install_roots()
        .into_iter()
        .find(|root| root.join("linux64").join("steamclient.so").is_file())
}

/// Copy `src` to `dest` only if `dest` does not already exist (idempotent
/// staging). Logs the outcome; a copy failure is a warning, never fatal.
#[cfg(target_os = "linux")]
fn copy_if_absent(src: &Path, dest: &Path) {
    if dest.exists() {
        log::info!("[PrefixPrep/B] {:?} already staged — leaving as-is", dest);
        return;
    }
    match std::fs::copy(src, dest) {
        Ok(bytes) => log::info!(
            "[PrefixPrep/B] Staged {:?} → {:?} ({} bytes)",
            src,
            dest,
            bytes
        ),
        Err(e) => log::warn!("[PrefixPrep/B] Failed to copy {:?} → {:?}: {}", src, dest, e),
    }
}

/// Bounded recursive search for the first file named `name` under `root`.
/// `max_depth` caps recursion so a pathological symlink loop or a huge tree
/// can't hang the launch. Returns the first match found (DFS order).
#[cfg(target_os = "linux")]
fn find_file_named(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    find_files_named(root, name, max_depth).into_iter().next()
}

/// Bounded recursive search collecting EVERY file named `name` under `root`.
/// Used where the caller must compare matches (e.g. newest-wins). Silently
/// skips unreadable dirs; respects `max_depth`.
#[cfg(target_os = "linux")]
fn find_files_named(root: &Path, name: &str, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    collect_files_named(root, name, max_depth, &mut out);
    out
}

/// Recursion worker for [`find_files_named`]. Does not follow into directories
/// once `max_depth` hits zero. Appends matches to `out`.
#[cfg(target_os = "linux")]
fn collect_files_named(dir: &Path, name: &str, max_depth: usize, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if max_depth > 0 {
                collect_files_named(&path, name, max_depth - 1, out);
            }
        } else if entry.file_name().to_string_lossy() == name {
            out.push(path);
        }
    }
}

/// Best-effort check for a running `steam` process by scanning `/proc/*/comm`.
/// Returns `false` on any read trouble (we only use this to emit an advisory
/// warning, never to block the launch).
#[cfg(target_os = "linux")]
fn is_steam_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        // Only numeric PID dirs have a useful comm file.
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let comm = entry.path().join("comm");
        if let Ok(name) = std::fs::read_to_string(&comm)
            && name.trim() == "steam"
        {
            return true;
        }
    }
    false
}

/// Resolve the user's home directory as a `String`, preferring `$HOME` and
/// falling back to [`std::env::home_dir`]. Returns an empty string if neither
/// is available (callers then build paths under an empty root, which simply
/// won't match anything — safe, since every file op here is best-effort).
#[cfg(target_os = "linux")]
fn home_dir_string() -> String {
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return home;
    }
    std::env::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}
