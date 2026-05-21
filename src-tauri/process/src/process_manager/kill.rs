//! Process-tree termination.
//!
//! Games are rarely a single process. A Proton/Wine title runs as
//! `bash → umu-run (python) → proton → wine → game.exe`; a Windows title is
//! often `launcher.exe → game.exe` (Unity/Unreal store launchers, EAC/BE
//! anti-cheat bootstrappers). Killing only the process Drop spawned leaves
//! the real game — and the GPU/audio resources it holds — orphaned.
//!
//! This module kills the *whole tree*:
//!
//!   - **Linux**: the child is `setsid()`-ed at spawn time into its own
//!     process group, so `kill(-pgid, …)` signals every descendant at once.
//!     SIGTERM first for a clean shutdown, SIGKILL after a grace period.
//!   - **Windows**: `taskkill /T /F` walks and force-kills the PID's child
//!     tree. `SharedChild::kill` alone would only terminate the launcher.
//!
//! Neither path blocks the caller — the launch-time wait thread observes the
//! real exit and runs [`super::exit`] cleanup. Killing is fire-and-forget.

use log::{info, warn};
use shared_child::SharedChild;

/// Grace period between the polite SIGTERM and the forced SIGKILL on Linux.
/// Long enough for Wine to flush, short enough that the UI feels responsive.
#[cfg(target_os = "linux")]
const TERM_TO_KILL_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

/// Terminate the entire process tree rooted at `child`.
///
/// Fire-and-forget: returns as soon as the kill signals are dispatched. The
/// launch-time wait thread detects the actual exit and performs cleanup.
/// `game_id` is only used for logging.
pub fn kill_process_tree(child: &SharedChild, game_id: &str) {
    let pid = child.id();

    #[cfg(target_os = "linux")]
    {
        // The child was placed in its own process group via setsid() at
        // spawn. Negating the pid targets the whole group, so umu/proton/
        // wine descendants are signalled too — not just bash.
        let pgid = pid as i32;
        info!("[KILL] {game_id}: SIGTERM -> process group {pgid}");
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }

        // Escalate to SIGKILL after a grace period, off the UI thread so a
        // slow Wine teardown never blocks the caller.
        std::thread::spawn(move || {
            std::thread::sleep(TERM_TO_KILL_GRACE);
            info!("[KILL] SIGKILL -> process group {pgid} (grace expired)");
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        });
    }

    #[cfg(target_os = "windows")]
    {
        // `child.kill()` would only terminate the launcher we spawned,
        // orphaning a game.exe started by a store/anti-cheat launcher.
        // `taskkill /T` walks the child tree; `/F` forces termination.
        info!("[KILL] {game_id}: taskkill /T /F pid {pid}");
        match std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(status) if status.success() => {
                info!("[KILL] {game_id}: taskkill succeeded");
            }
            Ok(status) => {
                // taskkill exits non-zero if the tree already died — fall
                // back to a direct kill so a launcher with no children is
                // still terminated.
                warn!(
                    "[KILL] {game_id}: taskkill exited {status}; falling back to direct kill"
                );
                let _ = child.kill();
            }
            Err(e) => {
                warn!("[KILL] {game_id}: could not spawn taskkill ({e}); direct kill");
                let _ = child.kill();
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        info!("[KILL] {game_id}: kill pid {pid}");
        let _ = child.kill();
    }

    info!("[KILL] {game_id}: kill signals dispatched, returning immediately");
}
