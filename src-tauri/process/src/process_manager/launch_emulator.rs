//! Emulator launch-command construction and the ENOEXEC spawn retry.
//!
//! Split from `launch.rs` to keep the main step-by-step launch flow
//! readable: emulator launches are a meaningfully different code path (the
//! game is a ROM passed to a separate emulator executable, with `{rom}`
//! placeholder substitution and RetroArch core auto-resolution), and the
//! ENOEXEC retry is a rare Linux fallback that would otherwise bloat
//! step 8 of `launch_process_inner`.

use std::path::PathBuf;

use database::{Database, GameDownloadStatus, models::data::InstalledGameType};
use tauri::Emitter as _;

use crate::{
    error::ProcessError,
    parser::ParsedCommand,
    process_manager::{ProcessHandler, ProcessManager},
};

// The ENOEXEC bash-wrapper retry below is Linux-only; its imports are gated
// so a Windows build doesn't flag them as unused.
#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use log::{info, warn};
#[cfg(target_os = "linux")]
use crate::process_manager::env;

/// Borrowed view of a launch config's `emulator` field.
///
/// `database` does not re-export the concrete `LaunchConfigurationEmulator`
/// type under its public `data` module, so [`build_emulator_command`] takes
/// this borrowed triple instead — the caller in `launch.rs` destructures the
/// real struct into it.
#[derive(Clone, Copy)]
pub(crate) struct EmulatorRef<'a> {
    pub launch_id: &'a str,
    pub game_id: &'a str,
    pub version_id: &'a str,
}

impl ProcessManager<'_> {
    /// Build the launch command for an **emulator** game.
    ///
    /// `emulator` is the `(launch_id, game_id, version_id)` reference triple
    /// from the ROM's launch config (the concrete
    /// `LaunchConfigurationEmulator` type is not re-exported by `database`,
    /// so the caller destructures it). `target_command` is the ROM-side
    /// parsed command (the ROM path lives in its `command` field). This
    /// resolves the emulator's own install dir + launch config, substitutes
    /// the ROM into the emulator command (`{rom}` placeholder, or
    /// `-L core rom` for RetroArch), generates an `.m3u` playlist for
    /// multi-disc titles, and hands the reconstructed emulator command to
    /// the platform handler.
    ///
    /// Sets `effective_cwd` to the emulator install dir (so relative emulator
    /// paths resolve) and `emulator_rom_path` to the ROM/playlist path.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_emulator_command(
        &self,
        db_lock: &Database,
        process_handler: &(dyn ProcessHandler + Send + Sync),
        game_id: &str,
        emulator: EmulatorRef<'_>,
        target_command: &mut ParsedCommand,
        install_dir: &str,
        disc_paths: &[String],
        effective_cwd: &mut Option<String>,
        emulator_rom_path: &mut Option<String>,
    ) -> Result<String, ProcessError> {
        let err = ProcessError::RequiredDependency(
            emulator.game_id.to_string(),
            emulator.version_id.to_string(),
        );

        let emulator_metadata = db_lock
            .applications
            .installed_game_version
            .get(emulator.game_id)
            .ok_or(err.clone())?;

        let emulator_game_status = db_lock
            .applications
            .game_statuses
            .get(emulator.game_id)
            .ok_or(err.clone())?;

        let emulator_install_dir = match emulator_game_status {
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::Installed,
                install_dir,
                ..
            } => Ok(install_dir),
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::SetupRequired,
                ..
            } => Err(ProcessError::InvalidArguments(
                "Complete emulator setup before launching games that use it.".to_string(),
            )),
            _ => Err(err.clone()),
        }?;

        *effective_cwd = Some(emulator_install_dir.clone());

        let emulator_game_version = db_lock
            .applications
            .game_versions
            .get(emulator.version_id)
            .ok_or(err.clone())?;

        let emulator_launch_config = emulator_game_version
            .launches
            .iter()
            .find(|v| v.launch_id == emulator.launch_id)
            .ok_or(err)?;

        let mut exe_command = ParsedCommand::parse(emulator_launch_config.command.clone())?;
        // Inject a fullscreen flag for known standalone emulators (Ryujinx,
        // Yuzu/Suyu, Cemu, standalone PCSX2). RetroArch is configured through
        // `configure_retroarch_for_game` and uses retroarch.cfg's
        // `video_fullscreen` key instead — its launcher is filtered out below
        // so we don't double-inject.
        inject_emulator_fullscreen_flag(&mut exe_command);
        exe_command
            .env
            .extend(std::mem::take(&mut target_command.env));
        exe_command.make_absolute(emulator_install_dir.into());
        target_command.make_absolute(PathBuf::from(install_dir));

        // Multi-disc titles get a generated .m3u playlist; single-disc just
        // uses the ROM path directly.
        let rom_path = if disc_paths.len() > 1 {
            let game_dir = std::path::Path::new(install_dir);
            crate::m3u::cleanup_m3u(game_dir);
            let m3u_path = crate::m3u::generate_m3u(game_dir, game_id, disc_paths)?;
            m3u_path.to_string_lossy().to_string()
        } else {
            target_command.command.clone()
        };
        *emulator_rom_path = Some(rom_path.clone());

        // Substitute the `{rom}` placeholder wherever it appears.
        let mut has_rom_placeholder = false;
        for arg in &mut exe_command.args {
            if arg.contains("{rom}") {
                *arg = arg.replace("{rom}", &rom_path);
                has_rom_placeholder = true;
            }
        }
        if exe_command.command.contains("{rom}") {
            exe_command.command = exe_command.command.replace("{rom}", &rom_path);
            has_rom_placeholder = true;
        }

        // No placeholder + a real ROM => append the ROM (and, for RetroArch,
        // auto-resolve the libretro core when none was specified).
        if !has_rom_placeholder && !rom_path.is_empty() {
            let has_core_flag = exe_command
                .args
                .iter()
                .any(|a| a == "-L" || a.starts_with("--libretro"));
            if !has_core_flag
                && let Some(core_path) = remote::retroarch::resolve_core_for_rom(
                    std::path::Path::new(emulator_install_dir),
                    &rom_path,
                ) {
                    exe_command.args.push("-L".to_string());
                    exe_command
                        .args
                        .push(core_path.to_string_lossy().to_string());
                }
            exe_command.args.push(rom_path.clone());
        }

        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "4_emulator_build",
            "game_id": game_id,
            "emulator_game_id": emulator.game_id,
            "rom_path": &rom_path,
            "has_rom_placeholder": has_rom_placeholder,
            "final_exe_command": &exe_command.command,
            "final_exe_args": &exe_command.args,
        }));

        let reconstructed = exe_command.reconstruct();
        process_handler.create_launch_process(
            emulator_metadata,
            reconstructed,
            emulator_game_version,
            emulator_install_dir,
            db_lock,
        )
    }

    /// Retry a failed `Command::spawn` under a `/bin/bash -c` wrapper when
    /// the failure was ENOEXEC and we didn't already wrap in bash.
    ///
    /// This handles pip-installed scripts (umu-run) that can't be detected
    /// as scripts via magic bytes — symlinks, compiled entry points,
    /// permission quirks. Outside Linux there is no retry: the original
    /// error is returned as-is.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn_with_enoexec_retry(
        &self,
        original_error: std::io::Error,
        game_id: &str,
        spawn_executable: &str,
        spawn_args: &[String],
        spawn_env: &[String],
        is_script: bool,
        working_dir: &str,
        log_path: &std::path::Path,
        error_log_path: &std::path::Path,
    ) -> Result<std::process::Child, ProcessError> {
        let emit_failure = |was_retry: bool, err: &std::io::Error| {
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "8_spawn_FAILED",
                "game_id": game_id,
                "error": format!("{err}"),
                "error_kind": format!("{:?}", err.kind()),
                "executable": spawn_executable,
                "executable_exists": std::path::Path::new(spawn_executable).exists(),
                "was_enoexec_retry": was_retry,
            }));
        };

        #[cfg(target_os = "linux")]
        {
            // ENOEXEC is errno 8.
            let is_enoexec = original_error.raw_os_error() == Some(8);
            if !is_enoexec || is_script {
                emit_failure(false, &original_error);
                return Err(original_error.into());
            }

            warn!("[LAUNCH] ENOEXEC on {spawn_executable:?} — retrying with bash wrapper");
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "8_enoexec_retry",
                "game_id": game_id,
                "original_error": format!("{original_error}"),
                "executable": spawn_executable,
            }));

            // Build a fully shell-quoted command string for `bash -c`.
            let mut shell_cmd = shell_words::quote(spawn_executable).to_string();
            for arg in spawn_args {
                shell_cmd.push(' ');
                shell_cmd.push_str(&shell_words::quote(arg));
            }

            let mut retry_cmd = Command::new("/bin/bash");
            retry_cmd.args(["-c", &shell_cmd]);
            env::apply_launch_env(&mut retry_cmd, spawn_env);
            retry_cmd
                .stderr(open_append_or_null(error_log_path))
                .stdout(open_append_or_null(log_path))
                .current_dir(working_dir);
            env::apply_baseline_env_scrub(&mut retry_cmd);
            env::sanitize_appimage_env(&mut retry_cmd);
            env::reapply_gamescope_env_for_retry(&mut retry_cmd, game_id);

            #[cfg(unix)]
            unsafe {
                use std::os::unix::process::CommandExt;
                retry_cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            match retry_cmd.spawn() {
                Ok(child) => {
                    info!("[LAUNCH] {game_id}: ENOEXEC retry spawned pid {}", child.id());
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "8_enoexec_retry_success", "game_id": game_id, "pid": child.id(),
                    }));
                    Ok(child)
                }
                Err(retry_err) => {
                    emit_failure(true, &retry_err);
                    Err(retry_err.into())
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (
                game_id,
                spawn_args,
                spawn_env,
                is_script,
                working_dir,
                log_path,
                error_log_path,
            );
            emit_failure(false, &original_error);
            Err(original_error.into())
        }
    }
}

/// Open `path` for appending, falling back to the null sink if it can't be
/// opened. Used for the ENOEXEC retry's stdio so a log-file hiccup never
/// blocks the retry itself.
#[cfg(target_os = "linux")]
fn open_append_or_null(path: &std::path::Path) -> std::process::Stdio {
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => std::process::Stdio::from(file),
        Err(_) => std::process::Stdio::null(),
    }
}

/// Inject the emulator-specific fullscreen flag into the launch command for
/// known standalone emulators. RetroArch is deliberately excluded — its
/// fullscreen toggle lives in retroarch.cfg (`video_fullscreen`) and is
/// handled by the per-game UserConfiguration path in `configure_retroarch_for_game`.
///
/// Matched by the basename of the executable (case-insensitive `contains`)
/// so symlinks, version-suffixed binaries, and AppImage wrappers all hit
/// the same branch. Skips injection if any common fullscreen-style flag is
/// already present, so a server-side launch command that already specifies
/// `--fullscreen` / `-fullscreen` / `-f` doesn't end up with a duplicate.
fn inject_emulator_fullscreen_flag(exe_command: &mut ParsedCommand) {
    let exe_lower = std::path::Path::new(&exe_command.command)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();

    // RetroArch is handled elsewhere — explicitly opt out so the rest of the
    // matching can use cheap `contains` checks without false positives.
    if exe_lower.contains("retroarch") {
        return;
    }

    // ryujinx/pcsx2 take `--fullscreen`; yuzu/suyu/cemu take `-f`. Grouped so
    // the shared flag values aren't duplicated across branches (clippy
    // if_same_then_else). Standalone PCSX2 only — the libretro core path goes
    // through RetroArch, handled above.
    let flag: &str = if exe_lower.contains("ryujinx") || exe_lower.contains("pcsx2") {
        "--fullscreen"
    } else if exe_lower.contains("yuzu")
        || exe_lower.contains("suyu")
        || exe_lower.contains("cemu")
    {
        "-f"
    } else {
        return;
    };

    let already_fullscreen = exe_command.args.iter().any(|a| {
        let a = a.as_str();
        a == "--fullscreen" || a == "-fullscreen" || a == "-f" || a == flag
    });
    if !already_fullscreen {
        // Prepend so it lands before the `{rom}` placeholder / appended ROM
        // path — argument order matters for some emulators that parse
        // positionally.
        exe_command.args.insert(0, flag.to_string());
    }
}
