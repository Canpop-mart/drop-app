//! Active co-op room tracking + `custom_broadcasts.txt` seeding/re-seeding.
//!
//! gbe_fork (Goldberg) discovers LAN peers by UDP broadcast, which ZeroTier's
//! L3 overlay drops. `steam_settings/custom_broadcasts.txt` lists peer IPs to
//! unicast the announce to instead — the glue that makes Goldberg co-op work
//! across a ZeroTier room.
//!
//! The bin crate records the active room here on host/join (and clears it on
//! leave). The launch path seeds the file just before spawning the game, AND a
//! background loop re-seeds it while the room is active — otherwise a peer who
//! joins *after* you launched is never added to your list, so you never announce
//! back to them and the session stays one-directional (the launch-order trap).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{info, warn};
use serde::Deserialize;

use crate::goldberg::write_custom_broadcasts;
use crate::requests::{generate_url, remote_request, NoBody, RemoteRequest};

/// The co-op room this client is currently in, if any (room id). Set by the bin
/// crate's `room_host`/`room_join`, cleared by `room_leave`.
static ACTIVE_ROOM: Mutex<Option<String>> = Mutex::new(None);

/// DLL dirs whose `custom_broadcasts.txt` we've seeded this room session. The
/// re-seed loop keeps them current as peers join, and `clear_active_room` clears
/// them on leave. One entry per launched co-op game — a Vec is plenty.
static SEEDED_DIRS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Record the room this client just hosted/joined so the launch path can seed
/// peer broadcasts for it.
pub fn set_active_room(room_id: &str) {
    *ACTIVE_ROOM.lock().unwrap() = Some(room_id.to_string());
    info!("[COOP] active room set: {room_id}");
}

/// The active room id, or None. The re-seed loop also uses this to detect a room
/// change (leave / rejoin) and stop itself.
pub fn current_room_id() -> Option<String> {
    ACTIVE_ROOM.lock().unwrap().clone()
}

/// Forget the active room and clear every seeded `custom_broadcasts.txt`, so a
/// later solo launch doesn't keep unicasting to stale peers.
pub fn clear_active_room() {
    *ACTIVE_ROOM.lock().unwrap() = None;
    let dirs: Vec<PathBuf> = std::mem::take(&mut *SEEDED_DIRS.lock().unwrap());
    for dir in &dirs {
        write_custom_broadcasts(dir, &[]);
    }
    info!(
        "[COOP] active room cleared ({} broadcast file(s) cleared)",
        dirs.len()
    );
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
    let Some(room_id) = current_room_id() else {
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

/// Seed a game's `custom_broadcasts.txt` at launch AND remember its DLL dir so
/// the re-seed loop keeps it current. An empty peer list (not in a room / no
/// peers assigned yet) clears the file and forgets the dir.
pub fn seed_and_record(dll_dir: &Path, peers: &[String]) {
    write_custom_broadcasts(dll_dir, peers);
    let mut seeded = SEEDED_DIRS.lock().unwrap();
    if peers.is_empty() {
        seeded.retain(|d| d != dll_dir);
    } else if !seeded.iter().any(|d| d == dll_dir) {
        seeded.push(dll_dir.to_path_buf());
    }
}

/// Re-seed every recorded game's `custom_broadcasts.txt` with the room's current
/// peers. Called on a timer while in a room so a late-joining peer is picked up
/// without a relaunch (removes the launch-order dependency). A transient empty
/// fetch is ignored so we never clobber a good seed; no-op until a game has been
/// seeded this session.
pub async fn reseed_all() {
    if current_room_id().is_none() {
        return;
    }
    let dirs: Vec<PathBuf> = SEEDED_DIRS.lock().unwrap().clone();
    if dirs.is_empty() {
        return;
    }
    let peers = current_peer_ips().await;
    if peers.is_empty() {
        return;
    }
    for dir in &dirs {
        write_custom_broadcasts(dir, &peers);
    }
}
