use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use database::DownloadableMetadata;
use log::{debug, error, info, warn};
use tauri::{AppHandle, async_runtime::JoinHandle};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use utils::{app_emit, lock, send};

use crate::{
    depot_manager::DepotManager,
    download_manager_frontend::DownloadStatus,
    error::ApplicationDownloadError,
    frontend_updates::{
        DownloadStatsUpdateEvent, QueueUpdateEvent, QueueUpdateEventQueueData,
    },
};

use super::{
    download_manager_frontend::{DownloadManager, DownloadManagerSignal, DownloadManagerStatus},
    downloadable::Downloadable,
    util::{
        download_thread_control_flag::{DownloadThreadControl, DownloadThreadControlFlag},
        progress_object::ProgressObject,
        queue::Queue,
    },
};

pub type DownloadAgent = Arc<Box<dyn Downloadable + Send + Sync>>;
pub type CurrentProgressObject = Arc<Mutex<Option<Arc<ProgressObject>>>>;

/*

Welcome to the download manager, the most overengineered, glorious piece of bullshit.

The download manager takes a queue of ids and their associated
DownloadAgents, and then, one-by-one, executes them. It provides an interface
to interact with the currently downloading agent, and manage the queue.

When the DownloadManager is initialised, it is designed to provide a reference
which can be used to provide some instructions (the DownloadManagerInterface),
but other than that, it runs without any sort of interruptions.

It does this by opening up two data structures. Primarily is the command_receiver,
and mpsc (multi-channel-single-producer) which allows commands to be sent from
the Interface, and queued up for the Manager to process.

These have been mapped in the DownloadManagerSignal docs.

The other way to interact with the DownloadManager is via the donwload_queue,
which is just a collection of ids which may be rearranged to suit
whichever download queue order is required.

+----------------------------------------------------------------------------+
| DO NOT ATTEMPT TO ADD OR REMOVE FROM THE QUEUE WITHOUT USING SIGNALS!!     |
| THIS WILL CAUSE A DESYNC BETWEEN THE DOWNLOAD AGENT REGISTRY AND THE QUEUE |
| WHICH HAS NOT BEEN ACCOUNTED FOR                                           |
+----------------------------------------------------------------------------+

This download queue does not actually own any of the DownloadAgents. It is
simply an id-based reference system. The actual Agents are stored in the
download_agent_registry HashMap, as ordering is no issue here. This is why
appending or removing from the download_queue must be done via signals.

Behold, my madness - quexeky

*/

pub struct DownloadManagerBuilder {
    download_agent_registry: HashMap<DownloadableMetadata, DownloadAgent>,
    download_queue: Queue,
    command_receiver: Receiver<DownloadManagerSignal>,
    sender: Sender<DownloadManagerSignal>,
    progress: CurrentProgressObject,
    status: Arc<Mutex<DownloadManagerStatus>>,
    app_handle: AppHandle,

    current_download_thread: Mutex<Option<JoinHandle<()>>>,
    active_control_flag: Option<DownloadThreadControl>,
}
impl DownloadManagerBuilder {
    pub fn build(app_handle: AppHandle) -> DownloadManager {
        let queue = Queue::new();
        let (command_sender, command_receiver) = mpsc::channel(1500);
        let active_progress = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(DownloadManagerStatus::Empty));

        let depot_manager = Arc::new(DepotManager::new());
        let manager = Self {
            download_agent_registry: HashMap::new(),
            download_queue: queue.clone(),
            command_receiver,
            status: status.clone(),
            sender: command_sender.clone(),
            progress: active_progress.clone(),
            app_handle,

            current_download_thread: Mutex::new(None),
            active_control_flag: None,
        };

        let terminator = tauri::async_runtime::spawn(async move {
            let result = manager.manage_queue().await;
            info!("download manager exited with result: {:?}", result);
        });

        DownloadManager::new(
            terminator,
            queue,
            active_progress,
            command_sender,
            depot_manager,
        )
    }

    fn set_status(&self, status: DownloadManagerStatus) {
        *lock!(self.status) = status;
        self.push_ui_queue_update();
    }

    async fn remove_and_cleanup_front_download(
        &mut self,
        meta: &DownloadableMetadata,
    ) {
        self.download_queue.pop_front();
        if self.download_agent_registry.remove(meta).is_none() {
            warn!("Attempted to remove download agent for {:?} but it was not in the registry", meta);
        }
        self.cleanup_current_download().await;
    }

    // CAREFUL WITH THIS FUNCTION
    // Make sure the download thread is terminated
    async fn cleanup_current_download(&mut self) {
        self.active_control_flag = None;
        *lock!(self.progress) = None;
        self.drain_previous_download(30).await;
    }

    /// Wait for the prior download thread (if any) to exit. If it doesn't
    /// exit within `timeout_secs`, abort it forcefully. Returns true if the
    /// thread drained on its own, false if it had to be aborted.
    ///
    /// Rationale: on pause, the thread's in-flight chunk futures keep
    /// running until the next control-flag check. With `max_download_threads`
    /// parallel chunks downloading large files, this drain can easily exceed
    /// the prior 4s cap. When the drain times out silently, a following
    /// resume sees stale agent state (status=Downloading, flag=Stop) and
    /// bails out — the user clicks resume but nothing happens, needing a
    /// second click after the orphaned thread eventually dies on its own.
    async fn drain_previous_download(&mut self, timeout_secs: u64) -> bool {
        let handle = {
            let mut download_thread_lock = lock!(self.current_download_thread);
            download_thread_lock.take()
        };
        let Some(mut handle) = handle else { return true };

        tokio::select! {
            _ = &mut handle => true,
            _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {
                warn!(
                    "previous download thread did not exit in {}s; aborting",
                    timeout_secs
                );
                handle.abort();
                // Await to let the abort propagate. JoinError is expected.
                let _ = (&mut handle).await;
                false
            }
        }
    }

    async fn stop_and_wait_current_download(&mut self) -> bool {
        self.set_status(DownloadManagerStatus::Paused);
        if let Some(current_flag) = &self.active_control_flag {
            current_flag.set(DownloadThreadControlFlag::Stop);
        }
        self.drain_previous_download(30).await
    }

    async fn manage_queue(mut self) -> Result<(), ()> {
        loop {
            let signal = match self.command_receiver.recv().await {
                Some(signal) => signal,
                None => return Err(()),
            };

            match signal {
                DownloadManagerSignal::Go => {
                    self.manage_go_signal().await;
                }
                DownloadManagerSignal::Stop => {
                    self.manage_stop_signal();
                }
                DownloadManagerSignal::Completed(meta) => {
                    self.manage_completed_signal(meta).await;
                }
                DownloadManagerSignal::Queue(download_agent) => {
                    self.manage_queue_signal(download_agent).await;
                }
                DownloadManagerSignal::Error(e) => {
                    self.manage_error_signal(e).await;
                }
                DownloadManagerSignal::UpdateUIQueue => {
                    self.push_ui_queue_update();
                }
                DownloadManagerSignal::UpdateUIDownloadStats(kbs, time) => {
                    self.push_ui_download_stats_update(kbs, time);
                }
                DownloadManagerSignal::Finish => {
                    self.stop_and_wait_current_download().await;
                    return Ok(());
                }
                DownloadManagerSignal::Cancel(meta) => {
                    self.manage_cancel_signal(&meta).await;
                }
            }
        }
    }
    async fn manage_queue_signal(&mut self, download_agent: DownloadAgent) {
        debug!("got signal Queue");
        let meta = download_agent.metadata();

        debug!("queue metadata: {meta:?}");

        if self.download_queue.exists(meta.clone()) {
            warn!("download with same ID already exists");
            return;
        }

        download_agent.on_queued(&self.app_handle);
        self.download_queue.append(meta.clone());
        self.download_agent_registry.insert(meta, download_agent);

        send!(self.sender, DownloadManagerSignal::UpdateUIQueue);
    }

    async fn manage_go_signal(&mut self) {
        debug!("got signal Go");
        if self.download_agent_registry.is_empty() {
            debug!(
                "Download agent registry: {:?}",
                self.download_agent_registry.len()
            );
            return;
        }

        debug!("current download queue: {:?}", self.download_queue.read());

        // Decide what to do based on the prior download's state:
        //   - flag=Stop → previous was paused, drain its thread before
        //     starting a new one (prevents a second concurrent downloader).
        //   - flag=Go → a download is actively running; leave it alone.
        //   - no active flag → fresh start, nothing to drain.
        match self.active_control_flag.as_ref().map(|f| f.get()) {
            Some(DownloadThreadControlFlag::Stop) => {
                self.drain_previous_download(30).await;
            }
            Some(DownloadThreadControlFlag::Go) => {
                debug!("Go received while download already active — ignoring");
                return;
            }
            None => {}
        }

        let agent_data = if let Some(agent_data) = self.download_queue.read().front() {
            agent_data.clone()
        } else {
            return;
        };

        let download_agent = match self.download_agent_registry.get(&agent_data) {
            Some(agent) => agent.clone(),
            None => {
                warn!("Download agent for {:?} not found in registry", &agent_data);
                return;
            }
        };

        // After draining, force status back to Queued. The thread normally
        // calls on_queued() itself before exiting via the Stop path, but if
        // drain_previous_download had to abort it, that cleanup never ran —
        // status would still be Downloading/Validating and a subsequent
        // status check would silently block the resume.
        if download_agent.status() != DownloadStatus::Queued {
            download_agent.on_queued(&self.app_handle);
        }

        // Ensure all others are marked as queued
        for agent in self.download_agent_registry.values() {
            if agent.metadata() != agent_data && agent.status() != DownloadStatus::Queued {
                agent.on_queued(&self.app_handle);
            }
        }

        info!("starting download for {agent_data:?}");
        self.active_control_flag = Some(download_agent.control_flag());

        // Set the flag to Go BEFORE spawning the new thread. Otherwise, if
        // the agent's flag is still Stop from a prior pause, the freshly
        // spawned task can observe Stop on its first control_flag check and
        // exit immediately.
        download_agent
            .control_flag()
            .set(DownloadThreadControlFlag::Go);

        let sender = self.sender.clone();

        let mut download_thread_lock = lock!(self.current_download_thread);
        let app_handle = self.app_handle.clone();

        *download_thread_lock = Some(tauri::async_runtime::spawn(async move {
            loop {
                let download_result = match download_agent.download(&app_handle).await {
                    // Ok(true) is for completed and exited properly
                    Ok(v) => v,
                    Err(e) => {
                        error!("download {:?} has error {}", download_agent.metadata(), &e);
                        download_agent.on_error(&app_handle, &e);
                        send!(sender, DownloadManagerSignal::Error(e));
                        return;
                    }
                };

                // If the download gets canceled or paused
                // If paused (Stop flag set), reset status to Queued so a later
                // Go signal can re-spawn this thread. If cancelled, on_cancelled
                // was called earlier and status is already set appropriately.
                if !download_result {
                    if download_agent.control_flag().get() == DownloadThreadControlFlag::Stop {
                        download_agent.on_queued(&app_handle);
                    }
                    return;
                }

                if download_agent.control_flag().get() == DownloadThreadControlFlag::Stop {
                    download_agent.on_queued(&app_handle);
                    return;
                }

                let validate_result = match download_agent.validate(&app_handle) {
                    Ok(v) => v,
                    Err(e) => {
                        error!(
                            "download {:?} has validation error {}",
                            download_agent.metadata(),
                            &e
                        );
                        download_agent.on_error(&app_handle, &e);
                        send!(sender, DownloadManagerSignal::Error(e));
                        return;
                    }
                };

                if download_agent.control_flag().get() == DownloadThreadControlFlag::Stop {
                    download_agent.on_queued(&app_handle);
                    return;
                }

                if validate_result {
                    download_agent.on_complete(&app_handle).await;
                    send!(
                        sender,
                        DownloadManagerSignal::Completed(download_agent.metadata())
                    );
                    send!(sender, DownloadManagerSignal::UpdateUIQueue);
                    return;
                }
            }
        }));

        self.set_status(DownloadManagerStatus::Downloading);
    }
    fn manage_stop_signal(&mut self) {
        if let Some(active_control_flag) = self.active_control_flag.clone() {
            self.set_status(DownloadManagerStatus::Paused);
            active_control_flag.set(DownloadThreadControlFlag::Stop);
        }
    }
    async fn manage_completed_signal(&mut self, meta: DownloadableMetadata) {
        if let Some(interface) = self.download_queue.read().front()
            && interface == &meta
        {
            self.remove_and_cleanup_front_download(&meta).await;
        }

        // Distinct from `update_queue` so the frontend can tell a real
        // completion apart from a cancellation (which also shrinks the queue).
        app_emit!(&self.app_handle, "download_complete", &meta.id);

        self.push_ui_queue_update();
        send!(self.sender, DownloadManagerSignal::Go);
    }
    async fn manage_error_signal(&mut self, error: ApplicationDownloadError) {
        warn!("got signal Error");
        if let Some(metadata) = self.download_queue.read().front()
            && let Some(current_agent) = self.download_agent_registry.get(metadata)
        {
            current_agent.on_error(&self.app_handle, &error);

            self.stop_and_wait_current_download().await;
            self.remove_and_cleanup_front_download(metadata).await;
        }
        self.push_ui_queue_update();
        self.set_status(DownloadManagerStatus::Error);
    }
    async fn manage_cancel_signal(&mut self, meta: &DownloadableMetadata) {
        // If the current download is the one we're tryna cancel
        if let Some(current_metadata) = self.download_queue.read().front()
            && current_metadata == meta
            && let Some(current_download) = self.download_agent_registry.get(current_metadata)
        {
            self.set_status(DownloadManagerStatus::Paused);
            current_download.on_cancelled(&self.app_handle);
            self.stop_and_wait_current_download().await;
            self.set_status(DownloadManagerStatus::Empty);

            self.download_queue.pop_front();

            self.cleanup_current_download().await;
            self.download_agent_registry.remove(meta);
        }
        // else just cancel it
        else if let Some(download_agent) = self.download_agent_registry.get(meta) {
            let index = self.download_queue.get_by_meta(meta);
            if let Some(index) = index {
                download_agent.on_cancelled(&self.app_handle);
                let _ = self.download_queue.edit().remove(index);
                let removed = self.download_agent_registry.remove(meta);
                debug!(
                    "removed {:?} from queue {:?}",
                    removed.map(|x| x.metadata()),
                    self.download_queue.read()
                );
            }
        }
        self.push_ui_queue_update();
        send!(self.sender, DownloadManagerSignal::Go);
    }
    fn push_ui_download_stats_update(&self, kbs: usize, time: usize) {
        let event_data = DownloadStatsUpdateEvent { speed: kbs, time };
        app_emit!(&self.app_handle, "update_stats", event_data);
    }
    fn push_ui_queue_update(&self) {
        let queue = &self.download_queue.read();
        let queue_objs = queue
            .iter()
            .filter_map(|key| {
                let val = self.download_agent_registry.get(key)?;
                Some(QueueUpdateEventQueueData {
                    meta: DownloadableMetadata::clone(key),
                    status: val.status(),
                    dl_progress: val.dl_progress().get_progress(),
                    dl_current: val.dl_progress().sum(),
                    dl_max: val.dl_progress().get_max(),
                    disk_progress: val.disk_progress().get_progress(),
                    disk_current: val.disk_progress().sum(),
                    disk_max: val.disk_progress().get_max(),
                })
            })
            .collect();

        let status = lock!(self.status).clone();
        let event_data = QueueUpdateEvent {
            queue: queue_objs,
            status,
        };
        app_emit!(&self.app_handle, "update_queue", event_data);
    }
}
