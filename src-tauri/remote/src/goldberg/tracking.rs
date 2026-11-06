//! Read-only source adapters. Never configure or replace game files.
use super::{EmulatorInfo, SteamEmulator, crackers};
use notify::{RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Notify;

#[derive(Clone, Debug)]
pub struct Sample {
    pub app_id: String,
    pub account: String,
    pub source: String,
    pub name: String,
    pub earned: bool,
    pub earned_time: u64,
    pub current: Option<f64>,
    pub target: Option<f64>,
}
impl Sample {
    pub fn key(&self) -> String {
        format!("{}\0{}\0{}", self.app_id, self.account, self.name)
    }
}
pub struct Snapshot {
    pub samples: Vec<Sample>,
    pub paths: Vec<PathBuf>,
    pub errors: Vec<String>,
}

fn json_samples(text: &str) -> Result<Vec<(String, bool, u64, Option<f64>, Option<f64>)>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let entries: Vec<(String, &serde_json::Value)> = if let Some(map) = value.as_object() {
        map.iter().map(|(k, v)| (k.clone(), v)).collect()
    } else if let Some(list) = value.as_array() {
        list.iter()
            .filter_map(|v| Some((v.get("name")?.as_str()?.to_string(), v)))
            .collect()
    } else {
        return Err("Unsupported achievement JSON shape".into());
    };
    Ok(entries
        .into_iter()
        .filter_map(|(name, v)| {
            if v.get("earned").is_none() {
                return None;
            }
            Some((
                name,
                v.get("earned").and_then(|v| v.as_bool()).unwrap_or(false),
                v.get("earned_time").and_then(|v| v.as_u64()).unwrap_or(0),
                v.get("progress").and_then(|v| v.as_f64()),
                v.get("max_progress")
                    .or_else(|| v.get("maxProgress"))
                    .and_then(|v| v.as_f64()),
            ))
        })
        .collect())
}

pub fn snapshot(
    app_ids: &[String],
    info: Option<&EmulatorInfo>,
    prefix: Option<&Path>,
) -> Snapshot {
    let mut out = Snapshot {
        samples: vec![],
        paths: vec![],
        errors: vec![],
    };
    for app_id in app_ids {
        let mut json = super::gse_candidate_paths(app_id, info.map(|i| i.dll_dir()), prefix);
        if let Some(info) = info {
            json.push(
                Path::new(info.dll_dir())
                    .join(super::DROP_GSE_FOLDER)
                    .join(app_id)
                    .join("achievements.json"),
            );
        }
        let mut sources = Vec::new();
        for roots in super::save_roots(prefix) {
            sources.extend(crackers::cracker_candidate_paths(app_id, &roots));
        }
        if let Some(info) = info {
            sources.extend(crackers::exe_adjacent_candidate_paths(info.dll_dir()));
        }
        for path in json {
            out.paths.push(path.clone());
            match fs::read_to_string(&path) {
                Ok(text) => match json_samples(&text) {
                    Ok(samples) => {
                        for (name, earned, earned_time, current, target) in samples {
                            out.samples.push(Sample {
                                app_id: app_id.clone(),
                                account: "local".into(),
                                source: "goldberg-json".into(),
                                name,
                                earned,
                                earned_time,
                                current,
                                target,
                            });
                        }
                    }
                    Err(e) => out.errors.push(format!("{}: {e}", path.display())),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => out.errors.push(format!("{}: {e}", path.display())),
            }
        }
        for source in sources {
            out.paths.push(source.path.clone());
            if source.path.exists() {
                match fs::read_to_string(&source.path) {
                    Ok(text) if !text.trim().is_empty() => {
                        let well_formed = match source.format {
                            crackers::CrackerFormat::GoldbergJson
                            | crackers::CrackerFormat::SteamCache => {
                                serde_json::from_str::<serde_json::Value>(&text).is_ok()
                            }
                            crackers::CrackerFormat::Razor1911 => true,
                            _ => text.contains('[') && text.contains(']'),
                        };
                        if !well_formed {
                            out.errors.push(format!(
                                "{}: unsupported or incomplete achievement file",
                                source.path.display()
                            ));
                            continue;
                        }
                        for a in crackers::parse_source(&source) {
                            out.samples.push(Sample {
                                app_id: app_id.clone(),
                                account: "local".into(),
                                source: source.label.into(),
                                name: a.name,
                                earned: a.earned,
                                earned_time: a.earned_time,
                                current: None,
                                target: None,
                            });
                        }
                    }
                    Ok(_) => out.errors.push(format!(
                        "{}: empty or incomplete achievement file",
                        source.path.display()
                    )),
                    Err(e) => out.errors.push(format!("{}: {e}", source.path.display())),
                }
            }
        }
        if let Some(EmulatorInfo {
            emulator: SteamEmulator::SmartSteamEmu { save_path, .. },
        }) = info
        {
            out.paths.push(save_path.join("achievements.ini"));
            for a in super::sse::read_sse_unlocks(save_path, app_id) {
                out.samples.push(Sample {
                    app_id: app_id.clone(),
                    account: "local".into(),
                    source: "smartsteamemu".into(),
                    name: a.name,
                    earned: a.earned,
                    earned_time: a.earned_time,
                    current: None,
                    target: None,
                });
            }
        }
        let native = super::steam::snapshot(app_id);
        out.samples.extend(native.samples);
        out.paths.extend(native.paths);
        out.errors.extend(native.errors);
    }
    out.paths.sort();
    out.paths.dedup();
    out
}

pub struct SourceWatch {
    watcher: Option<notify::RecommendedWatcher>,
    roots: HashSet<PathBuf>,
    pub signal: Arc<Notify>,
    pub error: Option<String>,
}
impl SourceWatch {
    pub fn new(paths: &[PathBuf]) -> Self {
        let signal = Arc::new(Notify::new());
        let wake = signal.clone();
        let created = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if !matches!(
                event,
                Ok(notify::Event {
                    kind: notify::EventKind::Access(_),
                    ..
                })
            ) {
                wake.notify_one();
            }
        });
        let (watcher, error) = match created {
            Ok(w) => (Some(w), None),
            Err(e) => (
                None,
                Some(format!(
                    "File notifications unavailable; using recovery scans: {e}"
                )),
            ),
        };
        let mut this = Self {
            watcher,
            roots: HashSet::new(),
            signal,
            error,
        };
        this.rearm(paths);
        this
    }
    /// Watch the closest existing parent. Recovery scans rearm as nested folders appear.
    pub fn rearm(&mut self, paths: &[PathBuf]) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        for path in paths {
            let mut parent = path.parent();
            while let Some(p) = parent {
                if p.is_dir() {
                    if !self.roots.contains(p) {
                        match watcher.watch(p, RecursiveMode::NonRecursive) {
                            Ok(()) => {
                                self.roots.insert(p.into());
                            }
                            Err(e) => {
                                self.error = Some(format!(
                                    "File notifications unavailable; using recovery scans: {e}"
                                ))
                            }
                        }
                    }
                    break;
                }
                parent = p.parent();
            }
        }
    }
}

pub fn progress_changed(
    previous: &HashMap<String, (Option<f64>, Option<f64>)>,
    sample: &Sample,
) -> bool {
    sample.current.is_some()
        && sample.target.is_some_and(|v| v > 0.0)
        && previous.get(&sample.key()) != Some(&(sample.current, sample.target))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_progress_and_partial_writes_are_distinct_from_definitions() {
        assert!(json_samples("{partial").is_err());
        assert!(
            json_samples(r#"[{"name":"first","displayName":"First"}]"#)
                .unwrap()
                .is_empty()
        );
        let s =
            json_samples(r#"{"first":{"earned":false,"progress":2,"max_progress":10}}"#).unwrap();
        assert_eq!(s[0].3, Some(2.0));
        assert_eq!(s[0].4, Some(10.0));
    }
    #[tokio::test]
    async fn watcher_recovers_when_a_save_directory_appears_and_files_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("late");
        let file = folder.join("achievements.json");
        let mut watch = SourceWatch::new(&[file.clone()]);
        assert!(watch.error.is_none());
        fs::create_dir(&folder).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), watch.signal.notified())
            .await
            .unwrap();
        watch.rearm(&[file.clone()]);
        fs::write(folder.join("new.json"), r#"{"first":{"earned":true}}"#).unwrap();
        fs::rename(folder.join("new.json"), &file).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), watch.signal.notified())
            .await
            .unwrap();
        assert!(json_samples(&fs::read_to_string(file).unwrap()).unwrap()[0].1);
    }
}
