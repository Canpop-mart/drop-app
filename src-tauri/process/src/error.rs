use std::{
    fmt::Display,
    io::{self, Error},
    sync::Arc,
};

use serde_with::SerializeDisplay;

/// Every way a game launch / process operation can fail.
///
/// Serialised to the frontend via [`SerializeDisplay`], so each variant's
/// `Display` text is user-facing and must be actionable. Variants that carry
/// a `String` wrap a foreign error with no typed form (`dynfmt` format
/// errors, `shell_words` parse errors) or an identifier the message needs
/// (the offending command, a dependency's id) — they are not stringly-typed
/// error handling.
#[derive(SerializeDisplay, Clone, Debug)]
pub enum ProcessError {
    /// The game is not installed (no install record, or only partially
    /// installed).
    NotInstalled,
    /// The game is already running — refused to launch a second instance.
    AlreadyRunning,
    /// The installed version has no matching `game_versions` entry.
    InvalidVersion,
    /// A game (e.g. an emulator) this launch depends on is not installed.
    /// Carries `(game_id, version_id)` so the UI can offer to install it.
    RequiredDependency(String, String),
    /// An underlying I/O failure (spawn, log-file creation, …).
    IOError(Arc<Error>),
    /// The user's launch template could not be expanded. Carries the
    /// `dynfmt` error text (no typed form available).
    FormatError(String),
    /// No launch handler exists for the current → target platform pair.
    InvalidPlatform,
    /// `tauri-plugin-opener` failed to open a path (e.g. the log directory).
    OpenerError(Arc<tauri_plugin_opener::Error>),
    /// A launch command could not be tokenised. Carries the offending
    /// command / `shell_words` parse error.
    InvalidArguments(String),
    /// A Windows game needs a Proton/UMU compat layer but none is configured.
    NoCompat,
    /// A native launch found a Windows binary and must fall back to compat.
    /// Carries the binary path. Handled internally as a fallback trigger.
    NeedsCompat(String),
    /// The resolved launch target doesn't exist on disk. Usual cause:
    /// antivirus (Defender) quarantining a game exe or crack DLL. Carries
    /// the missing path.
    LaunchTargetMissing(String),
}

impl Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProcessError::NotInstalled => "Game not installed",
            ProcessError::AlreadyRunning => "Game already running",
            ProcessError::InvalidVersion => "Invalid game version",
            ProcessError::IOError(error) => &error.to_string(),
            ProcessError::InvalidPlatform => "This game cannot be played on the current platform",
            ProcessError::FormatError(error) => &format!("Could not format template: {error:?}"),
            ProcessError::OpenerError(error) => &format!("Could not open directory: {error:?}"),
            ProcessError::InvalidArguments(arguments) => {
                &format!("Invalid arguments in command {arguments}")
            }
            ProcessError::RequiredDependency(game_id, version_id) => &format!(
                "Missing a required dependency to launch this game: {} {}",
                game_id, version_id
            ),
            ProcessError::NoCompat => "No Proton compatibility layer could be found for this tool. Add an override or set your global default in settings.",
            ProcessError::NeedsCompat(binary) => &format!(
                "Cannot run '{}' natively — this appears to be a Windows executable. \
                 It needs a compatibility layer (Proton/UMU). Check that the game's \
                 platform is set to Windows and that Proton is configured in settings.",
                binary
            ),
            ProcessError::LaunchTargetMissing(path) => &format!(
                "Launch file is missing: '{}'. It may have been removed by antivirus \
                 (Windows Defender). Verify or re-download the game, and consider adding \
                 it to your Defender exclusions in Settings.",
                path
            ),
        };
        write!(f, "{s}")
    }
}

impl From<io::Error> for ProcessError {
    fn from(value: io::Error) -> Self {
        ProcessError::IOError(Arc::new(value))
    }
}
