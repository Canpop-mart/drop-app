//! Proton-prefix preparation for umu/Proton launches.
//!
//! A freshly-created Proton prefix is bare. Some Windows games need the prefix
//! touched before they run, or they fail in ways that look like Drop bugs but
//! are actually missing runtime plumbing. Both recipes here were
//! reverse-engineered and confirmed working by hand on a Steam Deck.
//!
//! Everything is Linux-only — Windows/macOS keep no-op stubs so the workspace
//! still compiles with `#![deny]` lints and zero unused warnings.
//!
//! ## OnlineFix staging — automatic, at launch ([`prepare_prefix`])
//! OnlineFix payloads (online co-op cracks, the "Gunfire Reborn" category) ship
//! an `OnlineFix64.dll` next to the game exe and expect a live Steam install's
//! overlay DLL plus Proton's Steam bridge present at hardcoded Windows paths
//! inside the prefix. [`prepare_prefix`] detects the payload, stages those DLLs,
//! ensures the Linux-side `steamclient.so` symlinks exist, and returns the
//! `(KEY, VALUE)` override env the caller appends to the umu command. Detection
//! is a reliable *file-presence* check, so it runs automatically and NEVER
//! fails the launch: anything that goes wrong is logged at `warn`.
//!
//! ## VC++ runtime — manual, on demand ([`install_vcredist_into_prefix`])
//! Many Windows games link the MSVC 2015-2022 runtime (`VCRUNTIME140.dll`,
//! `MSVCP140.dll`), but the dependency is routinely imported by a sibling DLL
//! (`UnityPlayer.dll`, `GameAssembly.dll`) rather than the launch exe — so
//! reliably auto-detecting the need is a losing game. Instead the user triggers
//! it explicitly via the "Install VC++ Runtime" action. A fresh Proton prefix
//! has no runtime, and the raw `VC_redist.exe` does nothing under wine (its MSI
//! is blocked by wine's higher builtin `ucrtbase`); winetricks (which umu runs
//! natively) is the fix.

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

/// Prepare a Proton prefix for a umu launch: stage what an OnlineFix payload
/// needs. The VC++ runtime is handled separately and on demand by
/// [`install_vcredist_into_prefix`], not here.
///
/// * `install_dir` — the game's install directory (working dir of the launch).
/// * `exe_path`    — the absolute path to the launch executable. Used to locate
///   the OnlineFix payload directory (its own folder).
/// * `pfx_dir`     — the Proton prefix root (`WINEPREFIX`).
/// * `proton_path` — the resolved `PROTONPATH`: either an absolute Proton
///   install dir or a umu keyword like `GE-Proton` that umu auto-resolves.
///
/// Returns the extra `(KEY, VALUE)` env pairs to add to the umu command (the
/// OnlineFix DLL overrides). Empty when the game isn't an OnlineFix payload.
#[cfg(target_os = "linux")]
pub fn prepare_prefix(
    install_dir: &str,
    exe_path: &str,
    pfx_dir: &Path,
    proton_path: &str,
) -> Vec<(String, String)> {
    log::info!(
        "[PrefixPrep] Preparing prefix {:?} for exe {:?} (install_dir={:?})",
        pfx_dir,
        exe_path,
        install_dir
    );

    // OnlineFix: side effects + the override env the caller appends.
    let env = prepare_onlinefix(install_dir, exe_path, pfx_dir, proton_path);

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
    install_dir: &str,
    exe_path: &str,
    pfx_dir: &std::path::Path,
    proton_path: &str,
) -> Vec<(String, String)> {
    let _ = (install_dir, exe_path, pfx_dir, proton_path);
    Vec::new()
}

/// Emit a `game_prep_status` Tauri event so the game-detail UI can surface a
/// "Preparing…" status while a slow, blocking prefix-prep step runs (the first
/// `launch_game` / `install_vcredist` invoke doesn't return until prep
/// finishes, so without this the window looks frozen). `message: Some(_)` means
/// "prep active, show this text"; `None` means "prep done, clear the
/// indicator". No-op if the launch AppHandle hasn't been set yet (e.g. tests).
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

// ──────────────────────── VC++ runtime (on demand) ──────────────────────────

/// Install the MSVC 2015-2022 runtime (`vcrun2022`) plus `d3dcompiler_47` into
/// `pfx_dir` via umu's winetricks. Triggered by the user's "Install VC++
/// Runtime" action — there is no auto-detection, because the runtime is
/// routinely imported by a sibling DLL (`UnityPlayer.dll`, `GameAssembly.dll`)
/// rather than the launch exe, making a reliable sniff impossible.
///
/// Always does a real install (`--force`): GE-Proton pre-seeds stub VC++ redist
/// registry keys that would otherwise make winetricks skip and leave the redist
/// unregistered, which breaks games that check the registration. Blocking —
/// winetricks takes ~1 minute and needs network. Emits `game_prep_status` so the
/// UI can show progress for the duration. The `umu-run` child inherits a scrubbed
/// env (no `RUST_LOG`, no Steam/Gamescope bundled-Python vars that break umu's
/// system Python).
#[cfg(target_os = "linux")]
pub fn install_vcredist_into_prefix(
    game_id: &str,
    pfx_dir: &Path,
    proton_path: &str,
    umu_exe: &str,
) -> Result<(), String> {
    if let Err(e) = std::fs::create_dir_all(pfx_dir) {
        return Err(format!("Could not create prefix dir {pfx_dir:?}: {e}"));
    }

    log::info!(
        "[VCRedist] Installing vcrun2022 + d3dcompiler_47 into {:?} via winetricks \
         (umu, PROTONPATH={}). First run downloads from aka.ms and needs network.",
        pfx_dir,
        proton_path
    );

    // Run winetricks as umu's command (umu has native winetricks support):
    //   GAMEID=0 STORE=none PROTONPATH=<proton> WINEPREFIX=<pfx> \
    //       <umu-run> winetricks -q vcrun2022 d3dcompiler_47
    let mut command = std::process::Command::new(umu_exe);
    command
        .arg("winetricks")
        .arg("-q")
        // --force past GE-Proton's pre-seeded stub `VC\Runtimes` registry keys:
        // they fool winetricks into thinking vcrun2022 is already installed, so
        // it no-ops (a ~0.5s "success") and never writes the real redist
        // registration. Games that check that registration (e.g. Far Far West)
        // then refuse to launch ("missing Visual C++ 2015-2022 Redistributable")
        // even though the runtime DLLs are present. --force makes winetricks
        // install for real. Fine for a deliberate, on-demand button.
        .arg("--force")
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

    // Surface a precise status to the UI for the duration of the blocking run,
    // then clear it on every outcome below.
    emit_prep_status(
        game_id,
        Some("Installing Visual C++ runtime (one-time, ~1 min)..."),
    );
    let status_result = command.status();
    emit_prep_status(game_id, None);

    match status_result {
        Ok(status) if status.success() => {
            log::info!("[VCRedist] winetricks finished successfully for {pfx_dir:?}");
            Ok(())
        }
        Ok(status) => Err(format!(
            "winetricks exited with {status} — the VC++ runtime was not installed"
        )),
        Err(e) => Err(format!(
            "Failed to spawn winetricks via umu ({umu_exe:?}): {e}"
        )),
    }
}

/// Non-Linux stub: there is no Proton prefix to provision off Linux.
#[cfg(not(target_os = "linux"))]
pub fn install_vcredist_into_prefix(
    game_id: &str,
    pfx_dir: &std::path::Path,
    proton_path: &str,
    umu_exe: &str,
) -> Result<(), String> {
    let _ = (game_id, pfx_dir, proton_path, umu_exe);
    Err("Installing the VC++ runtime is only supported on Linux (Proton).".to_string())
}

// ───────────────────────────── OnlineFix ────────────────────────────────────

/// Stage everything an OnlineFix payload needs and return the canonical
/// DLL-override env set.
///
/// Detection: `OnlineFix64.dll` next to the launch exe, or — for UE-style games
/// that launch a thin root stub but nest the crack under `Binaries/Win64/` —
/// anywhere under the install dir. If absent this is not an OnlineFix game and we
/// return an empty env. When present, we (idempotently, where file ops involved):
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
fn prepare_onlinefix(
    install_dir: &str,
    exe_path: &str,
    pfx_dir: &Path,
    proton_path: &str,
) -> Vec<(String, String)> {
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

    // `OnlineFix64.dll` next to the launch exe is the common case (e.g. Gamble).
    // UE-style games launch a thin root stub (`Goofy.exe`) but ship the crack
    // nested at `…/Binaries/Win64/OnlineFix64.dll`, so fall back to a bounded
    // search of the install tree — without it the recipe silently never fires
    // for those games and OnlineFix can never register Spacewar.
    let onlinefix_dll = {
        let beside_exe = exe_dir.join("OnlineFix64.dll");
        if beside_exe.exists() {
            beside_exe
        } else if let Some(found) = find_file_named(Path::new(install_dir), "OnlineFix64.dll", 6) {
            found
        } else {
            log::info!(
                "[PrefixPrep/B] No OnlineFix64.dll next to the exe or under {:?} — not an OnlineFix game",
                install_dir
            );
            return Vec::new();
        }
    };

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
        Some(src) => stage_dll(&src, &steam_dir_in_pfx.join("GameOverlayRenderer64.dll")),
        None => log::warn!(
            "[PrefixPrep/B] Could not find GameOverlayRenderer64.dll in any Steam install — \
             OnlineFix overlay may fail with error 126"
        ),
    }

    // steamclient64.dll — Proton's lsteamclient.dll is the Windows→Linux Steam
    // bridge; OnlineFix looks for C:\Program Files (x86)\Steam\steamclient64.dll
    // which Proton doesn't populate under umu, so we copy it in under that name.
    match find_proton_lsteamclient(proton_path) {
        Some(src) => stage_dll(&src, &steam_dir_in_pfx.join("steamclient64.dll")),
        None => log::warn!(
            "[PrefixPrep/B] Could not find Proton's lsteamclient.dll (proton_path={:?}) — \
             OnlineFix Steam bridge will be missing",
            proton_path
        ),
    }

    // ── Step 2: ensure the Linux-side steamclient.so symlinks exist ──────────
    ensure_steam_sdk_symlinks();

    // ── Step 2b: force the OnlineFix Steam-detection sentinel ────────────────
    // OnlineFix reads HKCU\Software\Valve\Steam\ActiveProcess\PID and only accepts
    // Proton's 0xfffe "Steam is here" sentinel; a prefix carrying a real wine pid
    // makes it abort with "steam is not launched" and never load steamclient64.dll.
    ensure_steam_active_process_pid(pfx_dir);

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
///
/// Either way the result MUST be an `x86_64-windows` DLL: it is copied in as the
/// 64-bit `steamclient64.dll`, and a 32-bit (`i386-windows`) lsteamclient loaded
/// under that name fails with error 193 (bad image format). A Proton install
/// ships both arches under the same filename, so the keyword search filters.
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
            // A Proton install ships BOTH x86_64-windows/ and i386-windows/
            // copies under the same filename. Only the 64-bit one can be staged
            // as steamclient64.dll — loading the 32-bit one yields error 193
            // (bad image format). Never consider a non-x86_64 match.
            if !hit.to_string_lossy().contains("x86_64-windows") {
                continue;
            }
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
                "[PrefixPrep/B] Found newest x86_64 lsteamclient.dll via keyword search: {:?}",
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

/// Force `HKCU\Software\Valve\Steam\ActiveProcess\PID` to Proton's `0xfffe`
/// "Steam is here" sentinel inside the prefix's `user.reg`.
///
/// OnlineFix's "is Steam running?" check reads that value and rejects a real wine
/// pid, only accepting `0xfffe`; a prefix that ended up with a real pid (e.g.
/// written by an earlier non-Steam wine process) makes OnlineFix abort with
/// "steam is not launched" and never load `steamclient64.dll`. Proton does not
/// overwrite the value on subsequent launches, so a one-time rewrite sticks.
///
/// Best-effort and file-based (no extra wine launch): scoped to the
/// `ActiveProcess` section so no other key is touched, and silently skipped if
/// `user.reg` or that section isn't present yet (a brand-new prefix gets it on a
/// later launch, once Proton has initialised the registry). Never fails a launch.
#[cfg(target_os = "linux")]
fn ensure_steam_active_process_pid(pfx_dir: &Path) {
    const SECTION: &str = "[Software\\\\Valve\\\\Steam\\\\ActiveProcess]";
    const PID_KEY: &str = "\"PID\"=dword:";
    const SENTINEL: &str = "\"PID\"=dword:0000fffe";

    let reg_path = pfx_dir.join("user.reg");
    let content = match std::fs::read_to_string(&reg_path) {
        Ok(c) => c,
        Err(_) => {
            log::info!(
                "[PrefixPrep/B] user.reg not present yet — ActiveProcess PID will be set on a later launch"
            );
            return;
        }
    };
    if !content.contains(SECTION) {
        log::info!(
            "[PrefixPrep/B] No ActiveProcess section in user.reg yet — skipping the OnlineFix PID fix this launch"
        );
        return;
    }

    let mut out = String::with_capacity(content.len() + SENTINEL.len() + 1);
    let mut in_section = false;
    let mut pid_done = false;
    let mut changed = false;

    for line in content.lines() {
        if line.starts_with('[') {
            // Leaving a section: if it was ActiveProcess and had no PID line, add it.
            if in_section && !pid_done {
                out.push_str(SENTINEL);
                out.push('\n');
                changed = true;
            }
            in_section = line.starts_with(SECTION);
            pid_done = false;
        } else if in_section && !pid_done && line.starts_with(PID_KEY) {
            pid_done = true;
            if line != SENTINEL {
                out.push_str(SENTINEL);
                out.push('\n');
                changed = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    // The file may end while still inside the ActiveProcess section.
    if in_section && !pid_done {
        out.push_str(SENTINEL);
        out.push('\n');
        changed = true;
    }

    if !changed {
        log::info!("[PrefixPrep/B] ActiveProcess PID already the 0xfffe sentinel — no change");
        return;
    }
    match std::fs::write(&reg_path, &out) {
        Ok(()) => log::info!(
            "[PrefixPrep/B] Forced ActiveProcess\\PID to the 0xfffe sentinel (OnlineFix \"steam is not launched\" fix)"
        ),
        Err(e) => log::warn!(
            "[PrefixPrep/B] Could not rewrite user.reg for the PID fix: {} (launch continues)",
            e
        ),
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

/// Stage `src` to `dest`, ALWAYS overwriting any existing file.
///
/// The OnlineFix bridge DLLs are Drop-managed and MUST match the Proton the
/// game launches under. We deliberately do NOT keep a previously-staged copy:
/// an older Drop (which picked lsteamclient arch-blind) or a Proton upgrade can
/// leave a wrong-architecture or wrong-build `steamclient64.dll` that fails to
/// load with error 193. Re-copying every launch is cheap (a few MB) and lets a
/// poisoned prefix self-heal on the next launch. A copy failure is a warning,
/// never fatal.
#[cfg(target_os = "linux")]
fn stage_dll(src: &Path, dest: &Path) {
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
