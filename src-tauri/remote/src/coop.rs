//! Active co-op room tracking + `custom_broadcasts.txt` seeding/re-seeding.
//!
//! gbe_fork (Goldberg) discovers LAN peers by UDP broadcast, which ZeroTier's
//! L3 overlay drops. `steam_settings/custom_broadcasts.txt` lists peer IPs to
//! unicast the announce to instead — the glue that makes Goldberg co-op work
//! across a ZeroTier room.
//!
//! The bin crate records the active room here on host/join (and clears it on
//! leave). The launch path seeds the file just before spawning the game and
//! records the game's DLL dir even when nobody else has joined yet. After that
//! the file is kept current two ways: every member poll the UI makes
//! (`observe_peers`, called by the bin crate's `room_members`) and a slower
//! background loop (`reseed_all`).
//!
//! What that does and doesn't buy: gbe_fork reads `custom_broadcasts.txt` when
//! the game initialises the Steam API, so a game that is ALREADY running does
//! not see a rewrite. Keeping the file current means the next launch (or a
//! relaunch) of that game has every peer. For a running host, a peer who joins
//! later and then launches has the host's IP in its own file; its announce
//! reaches the host, which is how the host learns about it without a
//! relaunch. Nothing here has been shown to update a game mid-session.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use log::{info, warn};
use serde::Deserialize;

use crate::goldberg::write_custom_broadcasts;
use crate::requests::{generate_url, remote_request, NoBody, RemoteRequest};

/// The co-op room this client is currently in, if any (room id). Set by the bin
/// crate's `room_host`/`room_join`/`room_resume`, cleared by `room_leave`.
static ACTIVE_ROOM: Mutex<Option<String>> = Mutex::new(None);

/// Which DLL dirs have been seeded this room session and what each was last
/// written with.
static SEEDS: Mutex<SeedBook> = Mutex::new(SeedBook::new());

/// A poisoned lock here only means another thread panicked mid-update of a
/// plain list; the data is still usable, and co-op seeding must never take the
/// launch path down with it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pure bookkeeping for the seeded `custom_broadcasts.txt` files, kept apart
/// from the file writes so it can be unit-tested.
#[derive(Debug, Default, PartialEq)]
struct SeedBook {
    /// (DLL dir, the normalised peer list last written there).
    dirs: Vec<(PathBuf, Vec<String>)>,
}

/// Sort + dedup so "the same peers in a different order" is not a change.
fn normalise(peers: &[String]) -> Vec<String> {
    let mut v: Vec<String> = peers.iter().map(|p| p.trim().to_string()).collect();
    v.retain(|p| !p.is_empty());
    v.sort();
    v.dedup();
    v
}

impl SeedBook {
    const fn new() -> Self {
        Self { dirs: Vec::new() }
    }

    /// A game was just launched and its file written with `peers`. While in a
    /// room the dir is remembered even with zero peers, so the host who launches
    /// before anyone joins still has the file filled in later. Outside a room it
    /// is forgotten.
    fn record_launch(&mut self, dir: &Path, peers: &[String], in_room: bool) {
        self.dirs.retain(|(d, _)| d != dir);
        if in_room {
            self.dirs.push((dir.to_path_buf(), normalise(peers)));
        }
    }

    /// A fresh peer list arrived. Returns the dirs whose file must be rewritten
    /// (and marks them as written); the caller writes them while still holding
    /// the lock, so the book never runs ahead of the files.
    ///
    /// An empty list is ignored on purpose. The server omits a member whose IP
    /// the controller didn't return on that poll, so an empty list is often a
    /// hiccup, and clearing the file then could race a launch reading it. The
    /// cost of the other case (the last peer really left) is only announces
    /// sent to an address nobody uses; the file is cleared when we leave.
    fn observe(&mut self, peers: &[String]) -> Vec<PathBuf> {
        let peers = normalise(peers);
        if peers.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (dir, written) in &mut self.dirs {
            if *written != peers {
                *written = peers.clone();
                out.push(dir.clone());
            }
        }
        out
    }

    fn take_dirs(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.dirs)
            .into_iter()
            .map(|(d, _)| d)
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.dirs.is_empty()
    }
}

/// Record the room this client just hosted/joined so the launch path can seed
/// peer broadcasts for it.
pub fn set_active_room(room_id: &str) {
    *lock(&ACTIVE_ROOM) = Some(room_id.to_string());
    info!("[COOP] active room set: {room_id}");
}

/// The active room id, or None. The re-seed loop also uses this to detect a room
/// change (leave / rejoin) and stop itself.
pub fn current_room_id() -> Option<String> {
    lock(&ACTIVE_ROOM).clone()
}

/// Forget the active room and clear every seeded `custom_broadcasts.txt`, so a
/// later solo launch doesn't keep unicasting to stale peers.
pub fn clear_active_room() {
    *lock(&ACTIVE_ROOM) = None;
    let dirs = lock(&SEEDS).take_dirs();
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

/// Seed a game's `custom_broadcasts.txt` at launch AND, while in a room,
/// remember its DLL dir so later peer updates reach it. The dir is remembered
/// even when `peers` is empty (the host launched before anyone joined); outside
/// a room the file is cleared and the dir forgotten.
pub fn seed_and_record(dll_dir: &Path, peers: &[String]) {
    let in_room = current_room_id().is_some();
    // Same lock as observe_peers, so a poll can't interleave with this write.
    let mut book = lock(&SEEDS);
    write_custom_broadcasts(dll_dir, peers);
    book.record_launch(dll_dir, peers, in_room);
    drop(book);
    if in_room && peers.is_empty() {
        info!(
            "[COOP] no peers yet for {}; will fill in as they join",
            dll_dir.display()
        );
    }
}

/// Apply a freshly fetched peer list to every seeded game whose file is out of
/// date. Cheap when nothing changed (no file I/O). Called on every UI member
/// poll so the file is current within seconds for the game's next launch (a
/// running game read it at startup; see the module docs).
pub fn observe_peers(peers: &[String]) {
    if current_room_id().is_none() {
        return;
    }
    // Written under the lock: a 4s UI poll and the 12s loop can both land here,
    // and writing after unlocking could leave an older list in a file the book
    // already records as newer.
    let mut book = lock(&SEEDS);
    for dir in book.observe(peers) {
        write_custom_broadcasts(&dir, peers);
    }
}

/// Background fallback for `observe_peers`: fetch the room's peers and rewrite
/// any seeded file that is out of date. No-op until a game has been seeded this
/// room session.
pub async fn reseed_all() {
    if current_room_id().is_none() || lock(&SEEDS).is_empty() {
        return;
    }
    let peers = current_peer_ips().await;
    observe_peers(&peers);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn launch_with_no_peers_is_still_recorded_in_a_room() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/g"), &[], true);
        assert_eq!(b.dirs.len(), 1);
        // The first joiner then gets written.
        assert_eq!(b.observe(&s(&["10.242.1.2"])), vec![PathBuf::from("/g")]);
    }

    #[test]
    fn launch_outside_a_room_is_forgotten() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/g"), &s(&["10.0.0.1"]), true);
        b.record_launch(Path::new("/g"), &[], false);
        assert!(b.is_empty());
    }

    #[test]
    fn unchanged_peers_write_nothing_and_order_does_not_matter() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/g"), &s(&["10.0.0.2", "10.0.0.1"]), true);
        assert!(b.observe(&s(&["10.0.0.1", "10.0.0.2"])).is_empty());
        assert_eq!(
            b.observe(&s(&["10.0.0.1", "10.0.0.2", "10.0.0.3"])),
            vec![PathBuf::from("/g")]
        );
        assert!(b.observe(&s(&["10.0.0.3", "10.0.0.2", "10.0.0.1"])).is_empty());
    }

    #[test]
    fn empty_fetch_never_clobbers_a_seed() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/g"), &s(&["10.0.0.1"]), true);
        assert!(b.observe(&[]).is_empty());
        assert_eq!(b.dirs[0].1, s(&["10.0.0.1"]));
    }

    #[test]
    fn only_stale_dirs_are_rewritten() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/a"), &s(&["10.0.0.1"]), true);
        b.record_launch(Path::new("/b"), &[], true);
        assert_eq!(b.observe(&s(&["10.0.0.1"])), vec![PathBuf::from("/b")]);
    }

    #[test]
    fn relaunch_replaces_the_entry_instead_of_duplicating() {
        let mut b = SeedBook::new();
        b.record_launch(Path::new("/g"), &[], true);
        b.record_launch(Path::new("/g"), &s(&["10.0.0.1"]), true);
        assert_eq!(b.dirs.len(), 1);
        assert_eq!(b.take_dirs(), vec![PathBuf::from("/g")]);
        assert!(b.is_empty());
    }
}
