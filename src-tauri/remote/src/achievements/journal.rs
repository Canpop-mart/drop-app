//! Versioned sidecar storage. No change to drop.db, .dropdata or mod ledgers.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

static DISK_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub server: String,
    pub user_id: String,
    pub client_id: String,
}

impl Scope {
    pub fn current() -> Option<Self> {
        let user_id = crate::save_sync::current_user_id()?;
        let db = database::borrow_db_checked();
        let auth = db.auth.as_ref()?;
        if db.base_url.is_empty() {
            return None;
        }
        Some(Self {
            server: db.base_url.clone(),
            user_id,
            client_id: auth.client_id.clone(),
        })
    }

    pub fn is_current(&self) -> bool {
        Self::current().as_ref() == Some(self)
    }
    pub fn url(&self, path: &str) -> Result<url::Url, crate::error::RemoteAccessError> {
        Ok(url::Url::parse(&self.server)?.join(path)?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub event_id: String,
    pub external_id: String,
    pub provider: String,
    pub provider_game_id: String,
    pub provider_account: String,
    pub source: String,
    pub mode: String,
    pub kind: String,
    pub reset_epoch: String,
    pub observed_at: String,
    pub occurred_at: Option<String>,
    pub historical: bool,
    pub current: Option<f64>,
    pub target: Option<f64>,
}

impl Observation {
    pub fn identify(&mut self, scope: &Scope, game_id: &str) {
        // Unlock identity is stable through retries/restarts, but a reset creates a new identity.
        let key = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            scope.client_id,
            game_id,
            self.provider,
            self.provider_game_id,
            self.provider_account,
            self.external_id,
            self.mode,
            self.reset_epoch,
            self.kind,
            if self.kind == "progress" {
                self.observed_at.as_str()
            } else {
                ""
            }
        );
        self.event_id = format!("{:x}", md5::compute(key));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub version: u8,
    pub scope: Scope,
    pub game_id: String,
    pub event: Observation,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub achievement_id: Option<String>,
    pub disposition: String,
    pub reason: Option<String>,
    pub announced: bool,
}

#[derive(Clone)]
pub struct Journal {
    root: PathBuf,
    pub scope: Scope,
}

pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing journal parent"))?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut tmp, value).map_err(io::Error::other)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

impl Journal {
    pub fn open(scope: Scope) -> Self {
        Self::at(database::db::DATA_ROOT_DIR.join("achievements-v2"), scope)
    }
    pub fn at(root: PathBuf, scope: Scope) -> Self {
        let partition = format!(
            "{:x}",
            md5::compute(format!(
                "{}\0{}\0{}",
                scope.server, scope.user_id, scope.client_id
            ))
        );
        Self {
            root: root.join(partition),
            scope,
        }
    }
    fn path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }
    fn record_path(&self, record: &Record) -> PathBuf {
        if record.event.kind != "progress" {
            return self.path(&record.event.event_id);
        }
        let e = &record.event;
        let key = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.game_id,
            e.provider,
            e.provider_game_id,
            e.provider_account,
            e.external_id,
            e.mode,
            e.reset_epoch
        );
        self.root
            .join(format!("progress-{:x}.json", md5::compute(key)))
    }
    fn locate(&self, id: &str) -> io::Result<Option<PathBuf>> {
        let direct = self.path(id);
        if direct.exists() {
            return Ok(Some(direct));
        }
        // Progress has one latest-value file per identity. An old in-flight acknowledgement
        // must never mark a newer progress observation as synced.
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if !path
                .file_name()
                .is_some_and(|p| p.to_string_lossy().starts_with("progress-"))
            {
                continue;
            }
            let record: Record =
                serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
            if record.event.event_id == id {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    pub fn records(&self, game_id: Option<&str>) -> io::Result<Vec<Record>> {
        let _guard = DISK_LOCK
            .lock()
            .map_err(|_| io::Error::other("Achievement journal lock failed"))?;
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            // Preserve damaged files for recovery; never silently delete them or reset the journal.
            let record: Record = match serde_json::from_slice::<Record>(&fs::read(&path)?) {
                Ok(record) if record.version == 1 => record,
                _ => continue, // Isolate damaged entries; problems() exposes them without losing other events.
            };
            if record.scope == self.scope && game_id.is_none_or(|id| id == record.game_id) {
                records.push(record);
            }
        }
        records.sort_by(|a, b| a.event.observed_at.cmp(&b.event.observed_at));
        Ok(records)
    }

    pub fn problems(&self) -> io::Result<Vec<String>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let mut problems = vec![];
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match serde_json::from_slice::<Record>(&fs::read(&path)?) {
                Ok(record) if record.version == 1 => {}
                Ok(_) => problems.push(format!(
                    "Unsupported achievement journal version: {}",
                    path.display()
                )),
                Err(e) => problems.push(format!(
                    "Achievement record needs recovery: {} ({e})",
                    path.display()
                )),
            }
        }
        Ok(problems)
    }

    /// Returns true only for the first durable sighting.
    pub fn capture(&self, record: &Record) -> io::Result<bool> {
        let _guard = DISK_LOCK
            .lock()
            .map_err(|_| io::Error::other("Achievement journal lock failed"))?;
        let path = self.record_path(record);
        if path.exists() {
            if record.event.kind != "progress" {
                return Ok(false);
            }
            let existing: Record =
                serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
            if existing.event.observed_at >= record.event.observed_at {
                return Ok(false);
            }
        }
        atomic_json(&path, record)?;
        Ok(true)
    }

    pub fn acknowledge(
        &self,
        id: &str,
        status: &str,
        reason: Option<String>,
        achievement_id: Option<String>,
    ) -> io::Result<()> {
        let _guard = DISK_LOCK
            .lock()
            .map_err(|_| io::Error::other("Achievement journal lock failed"))?;
        let Some(path) = self.locate(id)? else {
            return Ok(());
        };
        let mut record: Record =
            serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
        record.disposition = status.to_string();
        record.reason = reason;
        if achievement_id.is_some() {
            record.achievement_id = achievement_id;
        }
        atomic_json(&path, &record)
    }

    pub fn announce_once(&self, id: &str) -> io::Result<bool> {
        let _guard = DISK_LOCK
            .lock()
            .map_err(|_| io::Error::other("Achievement journal lock failed"))?;
        let path = self.path(id);
        let mut record: Record =
            serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
        if record.announced || record.event.historical {
            return Ok(false);
        }
        // At most once presentation: a crash after this write can lose a popup, never the unlock.
        record.announced = true;
        atomic_json(&path, &record)?;
        Ok(true)
    }

    pub fn cache_config<T: Serialize>(&self, game_id: &str, config: &T) -> io::Result<()> {
        atomic_json(
            &self
                .root
                .join("configs")
                .join(format!("{:x}.json", md5::compute(game_id))),
            config,
        )
    }
    pub fn read_config<T: serde::de::DeserializeOwned>(&self, game_id: &str) -> io::Result<T> {
        serde_json::from_slice(&fs::read(
            self.root
                .join("configs")
                .join(format!("{:x}.json", md5::compute(game_id))),
        )?)
        .map_err(io::Error::other)
    }
    pub fn invalidate_config(&self, game_id: &str) -> io::Result<()> {
        match fs::remove_file(
            self.root
                .join("configs")
                .join(format!("{:x}.json", md5::compute(game_id))),
        ) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(user: &str) -> Scope {
        Scope {
            server: "server".into(),
            user_id: user.into(),
            client_id: "device".into(),
        }
    }
    fn record(scope: Scope) -> Record {
        let mut event = Observation {
            event_id: String::new(),
            external_id: "first".into(),
            provider: "Goldberg".into(),
            provider_game_id: "10".into(),
            provider_account: "local".into(),
            source: "fixture".into(),
            mode: "standard".into(),
            kind: "unlock".into(),
            reset_epoch: "initial".into(),
            observed_at: "2026-11-05T12:00:00Z".into(),
            occurred_at: None,
            historical: false,
            current: None,
            target: None,
        };
        event.identify(&scope, "game");
        Record {
            version: 1,
            scope,
            game_id: "game".into(),
            event,
            title: "First".into(),
            description: String::new(),
            icon_url: String::new(),
            achievement_id: None,
            disposition: "pending".into(),
            reason: None,
            announced: false,
        }
    }
    #[test]
    fn capture_survives_restart_and_account_switch_without_duplicate_popup() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::at(dir.path().into(), scope("alice"));
        let record = record(scope("alice"));
        assert!(journal.capture(&record).unwrap());
        assert!(journal.announce_once(&record.event.event_id).unwrap());
        let reopened = Journal::at(dir.path().into(), scope("alice"));
        assert!(!reopened.capture(&record).unwrap());
        assert!(!reopened.announce_once(&record.event.event_id).unwrap());
        assert_eq!(reopened.records(None).unwrap().len(), 1);
        assert!(
            Journal::at(dir.path().into(), scope("bob"))
                .records(None)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn missing_definitions_stay_pending_and_reset_creates_new_identity() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::at(dir.path().into(), scope("alice"));
        let mut record = record(scope("alice"));
        journal.capture(&record).unwrap();
        journal
            .acknowledge(
                &record.event.event_id,
                "awaiting_definitions",
                Some("definition_missing".into()),
                None,
            )
            .unwrap();
        assert_eq!(
            journal.records(None).unwrap()[0].disposition,
            "awaiting_definitions"
        );
        let old = record.event.event_id.clone();
        record.event.reset_epoch = "reset".into();
        record.event.identify(&record.scope, "game");
        assert_ne!(old, record.event.event_id);
        assert!(journal.capture(&record).unwrap());
    }
    #[test]
    fn corrupt_record_is_preserved_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::at(dir.path().into(), scope("alice"));
        let record = record(scope("alice"));
        journal.capture(&record).unwrap();
        fs::write(journal.path(&record.event.event_id), b"partial").unwrap();
        assert!(journal.records(None).unwrap().is_empty());
        assert_eq!(journal.problems().unwrap().len(), 1);
        assert!(journal.path(&record.event.event_id).exists());
    }
    #[test]
    fn progress_is_bounded_offline_and_old_ack_cannot_erase_latest_value() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::at(dir.path().into(), scope("alice"));
        let mut record = record(scope("alice"));
        record.event.kind = "progress".into();
        record.event.current = Some(1.0);
        record.event.target = Some(10.0);
        record.event.identify(&record.scope, "game");
        journal.capture(&record).unwrap();
        let old = record.event.event_id.clone();
        record.event.observed_at = "2026-11-05T12:00:01Z".into();
        record.event.current = Some(2.0);
        record.event.identify(&record.scope, "game");
        journal.capture(&record).unwrap();
        journal.acknowledge(&old, "accepted", None, None).unwrap();
        let rows = journal.records(None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event.current, Some(2.0));
        assert_eq!(rows[0].disposition, "pending");
    }
    #[test]
    fn old_settings_do_not_enable_popup_prototype_or_fail_to_deserialize() {
        let mut value = serde_json::to_value(database::Settings::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("achievementGamePopups");
        let old: database::Settings = serde_json::from_value(value).unwrap();
        assert!(!old.achievement_game_popups);
    }
}
