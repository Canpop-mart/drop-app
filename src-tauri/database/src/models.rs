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
    pub type UserConfiguration = v1::UserConfiguration;
    pub type MangoHudPreset = v1::MangoHudPreset;
    pub type ControllerType = v1::ControllerType;
    pub type QualityPreset = v1::QualityPreset;

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

    #[derive(Serialize, Deserialize)]
    enum DatabaseVersionEnum {
        V0_4_0 { database: v1::Database },
    }

    #[derive(Serialize)]
    enum DatabaseVersionEnumRef<'a> {
        V0_4_0 { database: &'a v1::Database },
    }

    pub struct DatabaseVersionSerializable(pub(crate) Database);

    impl DatabaseVersionSerializable {
        /// Serialize from a reference without cloning the database.
        pub fn serialize_ref<S>(database: &Database, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            DatabaseVersionEnumRef::V0_4_0 { database }.serialize(serializer)
        }

        /// Serialize the database to a RON string from a reference, avoiding a full clone.
        pub fn serialize_ref_to_ron(database: &Database) -> Result<String, ron::Error> {
            ron::to_string(&DatabaseVersionEnumRef::V0_4_0 { database })
        }
    }

    impl Serialize for DatabaseVersionSerializable {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            // Always serialize to latest version (zero-copy via reference)
            DatabaseVersionSerializable::serialize_ref(&self.0, serializer)
        }
    }

    impl<'de> Deserialize<'de> for DatabaseVersionSerializable {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(match DatabaseVersionEnum::deserialize(deserializer)? {
                DatabaseVersionEnum::V0_4_0 { database } => DatabaseVersionSerializable(database),
            })
        }
    }

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
                widescreen: false,
                mangohud: None,
            }
        }

        /// Controller layout type for RetroArch input mapping.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        pub enum ControllerType {
            Xbox,
            PlayStation,
            Nintendo,
        }

        /// Graphics quality preset for RetroArch.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        pub enum QualityPreset {
            Low,
            Medium,
            High,
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
            pub widescreen: bool,
            /// MangoHud performance overlay (Linux only)
            #[serde(default)]
            pub mangohud: Option<MangoHudPreset>,
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

        #[derive(Serialize, Deserialize, Clone, Debug)]
        #[serde(rename_all = "camelCase")]
        pub struct Settings {
            pub autostart: bool,
            pub max_download_threads: usize,
            pub force_offline: bool,
            /// Global MangoHud preset applied when a game has no per-game override.
            #[serde(default)]
            pub global_mangohud: Option<MangoHudPreset>,
        }
        impl Default for Settings {
            fn default() -> Self {
                Self {
                    autostart: false,
                    max_download_threads: 4,
                    force_offline: false,
                    global_mangohud: None,
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
