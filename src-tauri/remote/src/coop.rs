//! Active co-op room tracking for launch-time `custom_broadcasts.txt` seeding.
//!
//! gbe_fork (Goldberg) discovers LAN peers by UDP broadcast, which ZeroTier's
//! L3 overlay drops. `steam_settings/custom_broadcasts.txt` lists peer IPs to
//! unicast the announce to instead — the glue that makes Goldberg co-op work
//! across a ZeroTier room. The bin crate records the active room here on
//! host/join (and clears it on leave); the launch path reads the room's peer
//! IPs and seeds the file just before spawning the game.

use std::sync::Mutex;

use log::{info, warn};
use serde::Deserialize;

use crate::requests::{generate_url, remote_request, NoBody, RemoteRequest};

/// The co-op room this client is currently in, if any (room id). Set by the bin
/// crate's `room_host`/`room_join`, cleared by `room_leave`.
static ACTIVE_ROOM: Mutex<Option<String>> = Mutex::new(None);

/// Record the room this client just hosted/joined so the launch path can seed
/// peer broadcasts for it.
pub fn set_active_room(room_id: &str) {
    *ACTIVE_ROOM.lock().unwrap() = Some(room_id.to_string());
    info!("[COOP] active room set: {room_id}");
}

/// Forget the active room (on leave / session end) so a later solo launch
/// doesn't seed stale peer IPs.
pub fn clear_active_room() {
    *ACTIVE_ROOM.lock().unwrap() = None;
    info!("[COOP] active room cleared");
}

fn active_room_id() -> Option<String> {
    ACTIVE_ROOM.lock().unwrap().clone()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomDetail {
    /// The OTHER peers' overlay IPs (the server excludes us by client id).
    #[serde(default)]
    peer_addresses: Vec<String>,
}

/// The other peers' ZeroTier IPs for the active room, or an empty list when not
/// in a room or on any error. Co-op seeding is strictly best-effort and must
/// never block or fail a game launch, so every failure degrades to "no peers".
pub async fn current_peer_ips() -> Vec<String> {
    let Some(room_id) = active_room_id() else {
        return Vec::new();
    };
    let path = format!("/api/v1/client/room/{room_id}");
    let url = match generate_url(&[path.as_str()], &[]) {
        Ok(u) => u,
        Err(e) => {
            warn!("[COOP] could not build room URL: {e}");
            return Vec::new();
        }
    };
    // ZeroTier assigns each peer's overlay IP asynchronously after it joins, so
    // the room's peerAddresses can be briefly empty right after everyone joins.
    // Retry a few times so a game launched moments later still gets seeded. The
    // whole call is bounded by the caller's launch-time timeout, and a transport
    // error (not just an empty list) gives up immediately — co-op seeding must
    // never delay a launch.
    for attempt in 1..=3u32 {
        match remote_request::<RoomDetail, NoBody>(RemoteRequest::get(url.clone())).await {
            Ok(detail) if !detail.peer_addresses.is_empty() => {
                return detail.peer_addresses;
            }
            Ok(_) => {
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                }
            }
            Err(e) => {
                warn!("[COOP] could not fetch room peers: {e}");
                return Vec::new();
            }
        }
    }
    Vec::new()
}
