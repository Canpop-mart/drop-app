use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct User {
    id: String,
    username: String,
    admin: bool,
    display_name: String,
    profile_picture_object_id: String,
}

impl User {
    /// The user's chosen display name (shown in-game via GBE).
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn username(&self) -> &str {
        &self.username
    }
}
