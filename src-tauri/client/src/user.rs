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
    /// The account id the server keys everything by. Local save state is
    /// scoped by this so two Drop accounts sharing one PC cannot launder
    /// saves into each other's cloud library.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The user's chosen display name (shown in-game via GBE).
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn username(&self) -> &str {
        &self.username
    }
}
