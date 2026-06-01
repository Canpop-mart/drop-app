pub mod data {
    use std::{hash::Hash, path::PathBuf};

    use serde::{Deserialize, Serialize};

    // NOTE: Within each version, you should NEVER use these types.
    // Declare it using the actual version that it is from, i.e. v1::Settings rather than just Settings from here

    pub type Database = v1::Database;
    pub type GameVersion = v1::GameVersion;
    pub type Settings = v1::Settings;
    pub type DatabaseAuth = v1::DatabaseAuth;

    pub type GameDownloadStatus = v1::GameDownloadStatus;
    pub type InstalledGameType = v1::InstalledGameType;
    pub type ApplicationTransientStatus = v1::ApplicationTransientStatus;
    /**
     * Need to be universally accessible by the ID, and the version is just a couple sprinkles on top
     */
    pub type DownloadableMetadata = v1::DownloadableMetadata;
    pub type DownloadType = v1::DownloadType;
    pub type DatabaseApplications = v1::DatabaseApplications;
    pub type PendingQueueEntry = v1::PendingQueueEntry;
    pub type UserConfiguration = v1::UserConfiguration;
    pub type MangoHudPreset = v1::MangoHudPreset;
    pub type ControllerType = v1::ControllerType;
    pub type QualityPreset = v1::QualityPreset;
    pub type AspectRatio = v1::AspectRatio;

    use std::collections::HashMap;

    impl PartialEq for DownloadableMetadata {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id && self.download_type == other.download_type
        }
    }
    impl Hash for DownloadableMetadata {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state);
            self.download_type.hash(state);
        }
    }

    // The on-disk version envelope and migration chain now live in
    // `crate::migrations` (see `VersionedDatabase` / `migrate_to_latest`).
    // `interface.rs` serialises/deserialises through that module so every
    // schema bump goes through one documented place.

    mod v1 {
        use serde_with::serde_as;
        use std::{collections::HashMap, path::PathBuf};

        use crate::platform::Platform;

        use super::{Deserialize, Serialize};

        fn default_template() -> UserConfiguration {
            UserConfiguration {
                launch_template: "{}".to_owned(),
                override_proton_path: None,
                enable_updates: false,
                controller_type: None,
                quality_preset: None,
                widescreen: AspectRatio::Standard,
                mangohud: None,
                crt_shader: false,
                fullscreen: None,
            }
        }

        /// Controller layout type for RetroArch input mapping.
        ///
        /// Two real modes:
        ///   - **Xbox** — A=South, B=East, X=West, Y=North (RetroArch default)
        ///   - **Nintendo** — A=East, B=South, X=North, Y=West (A↔B, X↔Y swap via .rmp)
        ///
        /// `PlayStation` is kept only for backward-compatible deserialization of
        /// existing user databases; at runtime it behaves identically to `Xbox`.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        pub enum ControllerType {
            Xbox,
            /// Legacy variant — treated as Xbox at runtime.
            PlayStation,
            Nintendo,
        }

        /// Aspect ratio for emulated games.
        ///
        /// Backward-compatible with the old `widescreen: bool` field:
        /// `false` deserializes to `Standard`, `true` to `Wide16_9`.
        #[derive(Serialize, Clone, Debug, PartialEq, Eq)]
        pub enum AspectRatio {
            /// 4:3 — original console ratio (default)
            Standard,
            /// 16:9 — widescreen
            Wide16_9,
            /// 16:10 — Steam Deck / laptop displays
            Wide16_10,
        }

        impl Default for AspectRatio {
            fn default() -> Self {
                AspectRatio::Standard
            }
        }

        impl std::fmt::Display for AspectRatio {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    AspectRatio::Standard => write!(f, "4:3"),
                    AspectRatio::Wide16_9 => write!(f, "16:9"),
                    AspectRatio::Wide16_10 => write!(f, "16:10"),
                }
            }
        }

        // Custom deserialize to handle both old bool values and new enum strings
        impl<'de> serde::Deserialize<'de> for AspectRatio {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de;

                struct AspectRatioVisitor;

                impl<'de> de::Visitor<'de> for AspectRatioVisitor {
                    type Value = AspectRatio;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a boolean, unit, integer, map, or aspect ratio string")
                    }

                    fn visit_bool<E: de::Error>(self, v: bool) -> Result<AspectRatio, E> {
                        Ok(if v { AspectRatio::Wide16_9 } else { AspectRatio::Standard })
                    }

                    // Old database/dropdata files may store `widescreen: ()` (unit)
                    fn visit_unit<E: de::Error>(self) -> Result<AspectRatio, E> {
                        Ok(AspectRatio::Standard)
                    }

                    // Binary formats (pot) may encode enum variants as maps
                    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<AspectRatio, A::Error> {
                        // Try to read the variant name from the map key
                        let result = if let Some(key) = map.next_key::<String>()? {
                            let _: de::IgnoredAny = map.next_value()?;
                            match key.as_str() {
                                "Standard" => AspectRatio::Standard,
                                "Wide16_9" => AspectRatio::Wide16_9,
                                "Wide16_10" => AspectRatio::Wide16_10,
                                _ => AspectRatio::Standard,
                            }
                        } else {
                            AspectRatio::Standard
                        };
                        // Drain remaining entries
                        while let Some((_, _)) = map.next_entry::<de::IgnoredAny, de::IgnoredAny>()? {}
                        Ok(result)
                    }

                    // Binary formats may encode enum variant index as integer
                    fn visit_u64<E: de::Error>(self, v: u64) -> Result<AspectRatio, E> {
                        match v {
                            0 => Ok(AspectRatio::Standard),
                            1 => Ok(AspectRatio::Wide16_9),
                            2 => Ok(AspectRatio::Wide16_10),
                            _ => Ok(AspectRatio::Standard),
                        }
                    }

                    fn visit_str<E: de::Error>(self, v: &str) -> Result<AspectRatio, E> {
                        match v {
                            "Standard" => Ok(AspectRatio::Standard),
                            "Wide16_9" => Ok(AspectRatio::Wide16_9),
                            "Wide16_10" => Ok(AspectRatio::Wide16_10),
                            other => Err(de::Error::unknown_variant(
                                other,
                                &["Standard", "Wide16_9", "Wide16_10"],
                            )),
                        }
                    }
                }

                deserializer.deserialize_any(AspectRatioVisitor)
            }
        }

        /// Graphics quality preset for RetroArch.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        pub enum QualityPreset {
            Low,
            Medium,
            High,
            Ultra,
        }

        /// MangoHud overlay preset when launching games on Linux.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub enum MangoHudPreset {
            /// No overlay
            Off,
            /// FPS only
            Minimal,
            /// FPS + frametime + GPU/CPU usage
            Standard,
            /// All metrics
            Full,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub struct UserConfiguration {
            pub launch_template: String,
            pub override_proton_path: Option<String>,
            pub enable_updates: bool,
            #[serde(default)]
            pub controller_type: Option<ControllerType>,
            #[serde(default)]
            pub quality_preset: Option<QualityPreset>,
            #[serde(default)]
            pub widescreen: AspectRatio,
            /// Per-game fullscreen toggle for emulated games. `None` = use
            /// the RetroArch default (fullscreen on); `Some(false)` =
            /// explicit window mode. Old DBs that pre-date this field
            /// deserialize to `None` via `#[serde(default)]`.
            #[serde(default)]
            pub fullscreen: Option<bool>,
            /// MangoHud performance overlay (Linux only)
            #[serde(default)]
            pub mangohud: Option<MangoHudPreset>,
            /// Enable CRT shader overlay for emulated games (RetroArch)
            #[serde(default)]
            pub crt_shader: bool,
        }

        impl Default for UserConfiguration {
            fn default() -> Self {
                default_template()
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub struct GameVersion {
            pub game_id: String,
            pub version_id: String,

            pub display_name: Option<String>,
            pub version_path: String,

            pub only_setup: bool,

            pub version_index: usize,
            pub delta: bool,

            #[serde(default = "default_template")]
            pub user_configuration: UserConfiguration,

            pub launches: Vec<LaunchConfiguration>,
            pub setups: Vec<SetupConfiguration>,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub struct LaunchConfiguration {
            pub launch_id: String,

            pub name: String,
            pub command: String,
            pub platform: Platform,
            pub umu_id_override: Option<String>,

            pub emulator: Option<LaunchConfigurationEmulator>,

            /// Ordered disc image paths for multi-disc games (relative to game
            /// install dir). When this has 2+ entries the process manager writes
            /// a temporary .m3u playlist and passes it to the emulator via {rom}.
            #[serde(default)]
            pub disc_paths: Vec<String>,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        /**
         * This is intended to be used to look up the actual launch configuration that we store elsewhere
         */
        pub struct LaunchConfigurationEmulator {
            pub launch_id: String,
            pub game_id: String,
            pub version_id: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub struct SetupConfiguration {
            pub command: String,
            pub platform: Platform,
        }

        #[derive(Serialize, Deserialize, Clone)]
        #[serde(rename_all = "camelCase")]
        pub struct Settings {
            pub autostart: bool,
            pub max_download_threads: usize,
            pub force_offline: bool,
            /// Global MangoHud preset applied when a game has no per-game override.
            #[serde(default)]
            pub global_mangohud: Option<MangoHudPreset>,
            /// Sunshine admin username for streaming (defaults to "sunshine").
            #[serde(default = "default_sunshine_username")]
            pub sunshine_username: String,
            /// Sunshine admin password for streaming. Never logged via Debug.
            #[serde(default)]
            pub sunshine_password: String,
            /// RetroAchievements username for RetroArch cheevos auth. Empty
            /// means "fall back to server-linked account".
            #[serde(default)]
            pub ra_username: String,
            /// RetroAchievements Connect token (NOT a password). Never logged
            /// via Debug.
            #[serde(default)]
            pub ra_token: String,
            /// Global on/off switch for cloud save sync. When false, the
            /// pre-launch sync check and post-exit upload are both skipped.
            /// Defaults to true so existing users keep their current behaviour.
            #[serde(default = "default_true")]
            pub cloud_saves_enabled: bool,
            /// Friendly per-device label shown in the cloud save conflict UI
            /// (`uploadedFrom`). When `None` or empty, falls back to the raw
            /// hostname. Defaults to `None` for new installs.
            #[serde(default)]
            pub device_name: Option<String>,
            /// Game-streaming quality preset applied when this device connects
            /// to a host with Moonlight: `"dataSaver"`, `"balanced"`, or
            /// `"highQuality"`. Drives the fps/bitrate passed to Moonlight.
            #[serde(default = "default_streaming_quality")]
            pub streaming_quality: String,
            /// Game-streaming resolution: `"1280x800"` (Deck handheld, default),
            /// `"1920x1080"`, `"2560x1440"`, or `"native"` to leave the display
            /// unchanged. The host switches its display to this while streaming,
            /// and the client asks Moonlight to stream at it — set it bigger when
            /// the Deck is docked to a TV.
            #[serde(default = "default_streaming_resolution")]
            pub streaming_resolution: String,
        }

        // Manual Debug impl: redact the Sunshine password so it never leaks
        // via error reports, log lines, or panic messages.
        impl std::fmt::Debug for Settings {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Settings")
                    .field("autostart", &self.autostart)
                    .field("max_download_threads", &self.max_download_threads)
                    .field("force_offline", &self.force_offline)
                    .field("global_mangohud", &self.global_mangohud)
                    .field("sunshine_username", &self.sunshine_username)
                    .field(
                        "sunshine_password",
                        &if self.sunshine_password.is_empty() {
                            "<unset>"
                        } else {
                            "<redacted>"
                        },
                    )
                    .field("ra_username", &self.ra_username)
                    .field(
                        "ra_token",
                        &if self.ra_token.is_empty() {
                            "<unset>"
                        } else {
                            "<redacted>"
                        },
                    )
                    .field("cloud_saves_enabled", &self.cloud_saves_enabled)
                    .field("device_name", &self.device_name)
                    .field("streaming_quality", &self.streaming_quality)
                    .field("streaming_resolution", &self.streaming_resolution)
                    .finish()
            }
        }

        fn default_sunshine_username() -> String {
            "sunshine".to_string()
        }

        fn default_streaming_quality() -> String {
            "balanced".to_string()
        }

        fn default_streaming_resolution() -> String {
            "1280x800".to_string()
        }

        fn default_true() -> bool {
            true
        }

        impl Default for Settings {
            fn default() -> Self {
                Self {
                    autostart: false,
                    max_download_threads: 4,
                    force_offline: false,
                    global_mangohud: None,
                    sunshine_username: default_sunshine_username(),
                    sunshine_password: String::new(),
                    ra_username: String::new(),
                    ra_token: String::new(),
                    cloud_saves_enabled: default_true(),
                    device_name: None,
                    streaming_quality: default_streaming_quality(),
                    streaming_resolution: default_streaming_resolution(),
                }
            }
        }

        #[derive(Serialize, Clone, Deserialize, Debug)]
        #[serde(tag = "type")]
        pub enum InstalledGameType {
            SetupRequired,
            Installed,
            PartiallyInstalled {
                #[serde(skip)]
                configuration: UserConfiguration,
            },
        }

        #[derive(Serialize, Clone, Deserialize, Debug)]
        #[serde(tag = "type")]
        pub enum GameDownloadStatus {
            Remote {},
            Installed {
                install_type: InstalledGameType,
                version_id: String,
                install_dir: String,
                update_available: bool,
            },
        }
        // Stuff that shouldn't be synced to disk
        #[derive(Clone, Serialize, Deserialize, Debug)]
        #[serde(tag = "type")]
        pub enum ApplicationTransientStatus {
            Queued { version_id: String },
            Downloading { version_id: String },
            Uninstalling {},
            Updating { version_id: String },
            Validating { version_id: String },
            Running {},
        }

        #[derive(serde::Serialize, Clone, Deserialize)]
        pub struct DatabaseAuth {
            pub private: String,
            pub cert: String,
            pub client_id: String,
            pub web_token: Option<String>,
        }

        #[derive(
            Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Clone, Copy,
        )]
        pub enum DownloadType {
            Game,
            Tool,
            Dlc,
            Mod,
        }

        #[derive(Debug, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone)]
        #[serde(rename_all = "camelCase")]
        pub struct DownloadableMetadata {
            pub id: String,
            pub version: String,
            pub target_platform: Platform,
            pub download_type: DownloadType,
        }
        impl DownloadableMetadata {
            pub fn new(
                id: String,
                version: String,
                target_platform: Platform,
                download_type: DownloadType,
            ) -> Self {
                Self {
                    id,
                    version,
                    target_platform,
                    download_type,
                }
            }
        }

        /// Persisted snapshot of one queued download. The live `Queue`
        /// (in download_manager::util::queue) holds just `DownloadableMetadata`
        /// in order, but to recreate a `GameDownloadAgent` on startup we also
        /// need the install dir the user picked and whether updates were
        /// enabled — neither of which survives in `DownloadableMetadata`
        /// itself. `DatabaseApplications::pending_queue` stores these in
        /// insertion order; mutations to the live queue mirror onto it.
        #[derive(Debug, Serialize, Deserialize, Clone)]
        #[serde(rename_all = "camelCase")]
        pub struct PendingQueueEntry {
            pub meta: DownloadableMetadata,
            pub install_dir: usize,
            pub enable_updates: bool,
        }

        #[serde_as]
        #[derive(Serialize, Clone, Deserialize, Default)]
        #[serde(rename_all = "camelCase")]
        pub struct DatabaseApplications {
            pub install_dirs: Vec<PathBuf>,
            // Guaranteed to exist if the game also exists in the app state map
            pub game_statuses: HashMap<String, GameDownloadStatus>,

            pub game_versions: HashMap<String, GameVersion>,
            pub installed_game_version: HashMap<String, DownloadableMetadata>,

            pub additional_proton_paths: Vec<String>,
            pub default_proton_path: Option<String>,

            /// Downloads queued but not yet completed. Persisted so a crash
            /// or relaunch restores the queue rather than wiping it.
            /// `#[serde(default)]` keeps DBs written by older builds (which
            /// lacked this field) loadable.
            #[serde(default)]
            pub pending_queue: Vec<PendingQueueEntry>,

            #[serde(skip)]
            pub transient_statuses: HashMap<DownloadableMetadata, ApplicationTransientStatus>,
        }

        #[derive(Serialize, Deserialize, Clone, Default)]
        pub struct Database {
            #[serde(default)]
            pub settings: Settings,
            pub auth: Option<DatabaseAuth>,
            pub base_url: String,
            pub applications: DatabaseApplications,
            pub cache_dir: PathBuf,

            #[serde(skip)]
            pub prev_database: Option<PathBuf>,
        }
    }

    impl Database {
        pub fn new<T: Into<PathBuf>>(
            games_base_dir: T,
            prev_database: Option<PathBuf>,
            cache_dir: PathBuf,
        ) -> Self {
            Self {
                applications: DatabaseApplications {
                    install_dirs: vec![games_base_dir.into()],
                    game_statuses: HashMap::new(),
                    game_versions: HashMap::new(),
                    installed_game_version: HashMap::new(),
                    transient_statuses: HashMap::new(),
                    additional_proton_paths: Vec::new(),
                    default_proton_path: None,
                    pending_queue: Vec::new(),
                },
                prev_database,
                base_url: String::new(),
                auth: None,
                settings: Settings::default(),
                cache_dir,
            }
        }
    }
    impl DatabaseAuth {
        pub fn new(
            private: String,
            cert: String,
            client_id: String,
            web_token: Option<String>,
        ) -> Self {
            Self {
                private,
                cert,
                client_id,
                web_token,
            }
        }
    }
}
