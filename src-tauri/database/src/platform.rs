use serde::{Deserialize, Serialize};

#[derive(Eq, Hash, PartialEq, Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord)]
pub enum Platform {
    Windows,
    Linux,
    #[allow(non_camel_case_types)]
    macOS,
}

impl Platform {
    #[cfg(target_os = "windows")]
    pub const HOST: Platform = Self::Windows;
    #[cfg(target_os = "macos")]
    pub const HOST: Platform = Self::macOS;
    #[cfg(target_os = "linux")]
    pub const HOST: Platform = Self::Linux;

    pub fn is_case_sensitive(&self) -> bool {
        match self {
            Self::Windows | Self::macOS => false,
            Self::Linux => true,
        }
    }
}

impl From<&str> for Platform {
    fn from(value: &str) -> Self {
        match value.to_lowercase().trim() {
            "windows" | "win" | "win32" | "win64" => Self::Windows,
            "linux" => Self::Linux,
            "mac" | "macos" | "darwin" | "osx" => Self::macOS,
            other => panic!("unsupported platform string: {other:?}"),
        }
    }
}

impl From<whoami::Platform> for Platform {
    fn from(value: whoami::Platform) -> Self {
        match value {
            whoami::Platform::Windows => Platform::Windows,
            whoami::Platform::Linux => Platform::Linux,
            whoami::Platform::MacOS => Platform::macOS,
            whoami::Platform::Bsd
            | whoami::Platform::Illumos
            | whoami::Platform::Redox
            | whoami::Platform::Unknown(_) => {
                panic!("platform {value} is not supported")
            }
            // Handle any future variants added to the whoami crate
            _ => panic!("platform {value} is not supported"),
        }
    }
}
