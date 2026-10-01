use std::fmt::Display;

use serde_with::SerializeDisplay;

#[derive(SerializeDisplay)]
pub enum LibraryError {
    MetaNotFound(String),
    VersionNotFound(String),
    IsMod(String),
    /// The mod is queued or downloading, so its files are in use.
    ModBusy,
    /// The base game (or another of its mods) is queued, downloading or
    /// running, so the files a mod shares with it may be in use or mid-write.
    GameBusy,
    /// A mod's files could not be read or fully removed. The string says why.
    ModFiles(String),
}
impl Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LibraryError::MetaNotFound(id) => {
                    format!(
                        "Could not locate any installed version of game ID {id} in the database"
                    )
                }
                LibraryError::VersionNotFound(game_id) => {
                    format!(
                        "Could not locate any installed version  for game id {game_id} in the database"
                    )
                }
                LibraryError::IsMod(id) => {
                    format!(
                        "Game ID {id} is a mod. Remove it from the base game's Mods list instead."
                    )
                }
                LibraryError::ModBusy => {
                    "This mod is still downloading. Wait for it to finish or cancel it, then try again."
                        .to_string()
                }
                LibraryError::GameBusy => {
                    "The game or one of its mods is busy (queued, downloading or running). Let that finish, then try again."
                        .to_string()
                }
                LibraryError::ModFiles(why) => {
                    format!("Some of this mod's files could not be handled: {why}")
                }
            }
        )
    }
}
