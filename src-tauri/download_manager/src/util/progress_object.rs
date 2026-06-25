use std::{
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use atomic_instant_full::AtomicInstant;
use tokio::sync::mpsc::Sender;
use utils::{lock, send};

use crate::download_manager_frontend::DownloadManagerSignal;

use super::rolling_progress_updates::RollingProgressWindow;

#[derive(Clone, Debug)]
pub enum ProgressType {
    Download,
    Disk,
}

/// Sample interval (~250 ms) × this size = effective smoothing window.
/// 20 samples ≈ 5 s of history — long enough that a single TCP burst (where
/// the kernel socket buffer drains into one read of ~200 MB) washes out of
/// the displayed average within a few seconds, short enough that the
/// reading still tracks the real rate.
const SPEED_WINDOW_SAMPLES: usize = 20;

/// Hard cap on any single speed sample, in KB/s. 5 GB/s is well above
/// any realistic disk + network combination Drop will ever see; anything
/// past this is a measurement artefact (a burst read attributing several
/// hundred MB to a sub-second window) and would drag the average to
/// nonsense values. Clamp at the sample boundary so the rolling average
/// stays meaningful.
const SPEED_SAMPLE_CLAMP_KBS: usize = 5_000_000;

#[derive(Clone, Debug)]
pub struct ProgressObject {
    progress_type: ProgressType,
    max: Arc<Mutex<usize>>,
    progress_instances: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
    start: Arc<Mutex<Instant>>,
    sender: Sender<DownloadManagerSignal>,
    bytes_last_update: Arc<AtomicUsize>,
    rolling: RollingProgressWindow<SPEED_WINDOW_SAMPLES>,
}

#[derive(Clone)]
pub struct ProgressHandle {
    progress: Arc<AtomicUsize>,
    progress_object: Arc<ProgressObject>,
}

static LAST_UPDATE_TIME: LazyLock<AtomicInstant> = LazyLock::new(AtomicInstant::now);

impl ProgressHandle {
    pub fn new(progress: Arc<AtomicUsize>, progress_object: Arc<ProgressObject>) -> Self {
        Self {
            progress,
            progress_object,
        }
    }
    pub fn set(&self, amount: usize) {
        self.progress.store(amount, Ordering::Release);
    }
    pub fn add(&self, amount: usize) {
        self.progress
            .fetch_add(amount, std::sync::atomic::Ordering::AcqRel);
        spawn_update(&self.progress_object);
    }
    pub fn skip(&self, amount: usize) {
        self.progress
            .fetch_add(amount, std::sync::atomic::Ordering::Acquire);
        // Offset the bytes at last offset by this amount
        self.progress_object
            .bytes_last_update
            .fetch_add(amount, Ordering::Acquire);
        // Dont' fire update
    }
}

impl ProgressObject {
    pub fn new(
        max: usize,
        length: usize,
        sender: Sender<DownloadManagerSignal>,
        progress_type: ProgressType,
    ) -> Self {
        let arr = Mutex::new((0..length).map(|_| Arc::new(AtomicUsize::new(0))).collect());
        Self {
            max: Arc::new(Mutex::new(max)),
            progress_instances: Arc::new(arr),
            start: Arc::new(Mutex::new(Instant::now())),
            sender,

            bytes_last_update: Arc::new(AtomicUsize::new(0)),
            rolling: RollingProgressWindow::new(),
            progress_type,
        }
    }

    pub fn set_time_now(&self) {
        *lock!(self.start) = Instant::now();
    }
    pub fn sum(&self) -> usize {
        lock!(self.progress_instances)
            .iter()
            .map(|instance| instance.load(Ordering::Acquire))
            .sum()
    }
    pub fn reset(&self) {
        self.set_time_now();
        self.bytes_last_update.store(0, Ordering::Release);
        self.rolling.reset();
        lock!(self.progress_instances)
            .iter()
            .for_each(|x| x.store(0, Ordering::SeqCst));
    }
    pub fn get_max(&self) -> usize {
        *lock!(self.max)
    }
    pub fn set_max(&self, new_max: usize) {
        *lock!(self.max) = new_max;
    }
    pub fn set_size(&self, length: usize) {
        *lock!(self.progress_instances) =
            (0..length).map(|_| Arc::new(AtomicUsize::new(0))).collect();
    }
    pub fn get_progress(&self) -> f64 {
        let max = self.get_max();
        if max == 0 {
            return 0.0;
        }
        self.sum() as f64 / max as f64
    }
    pub fn get(&self, index: usize) -> Arc<AtomicUsize> {
        lock!(self.progress_instances)[index].clone()
    }
    fn update_window(&self, kilobytes_per_second: usize) {
        self.rolling.update(kilobytes_per_second);
    }
}

pub fn spawn_update(progress: &Arc<ProgressObject>) {
    // Many parallel chunk readers all call this on every read. Without the
    // CAS below, every reader would observe `time_since_last_update > 250ms`
    // and spawn calculate_update before any of them could update the
    // timestamp. Only the first one would see the real bytes delta via
    // bytes_last_update.swap(); the rest would record ~0 KB/s into the
    // rolling window, diluting the displayed average by ~1/N. The CAS
    // ensures exactly one task wins each window.
    let now = Instant::now();
    let last_update_time = LAST_UPDATE_TIME.load(Ordering::SeqCst);
    let time_since_last_update = now.duration_since(last_update_time).as_millis_f64();
    if time_since_last_update < 250.0 {
        return;
    }
    if LAST_UPDATE_TIME
        .compare_exchange(last_update_time, now, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(calculate_update(progress.clone(), time_since_last_update));
}

pub async fn calculate_update(progress: Arc<ProgressObject>, time_since_last_update: f64) {
    let current_bytes_downloaded = progress.sum();
    let max = progress.get_max();
    let bytes_at_last_update = progress
        .bytes_last_update
        .swap(current_bytes_downloaded, Ordering::Acquire);

    let bytes_since_last_update =
        current_bytes_downloaded.saturating_sub(bytes_at_last_update) as f64;

    // bytes / ms == KB / s (since 1 KB == 1000 B and 1 s == 1000 ms).
    let kilobytes_per_second = (bytes_since_last_update / time_since_last_update) as usize;

    // Clamp before pushing into the rolling window — a single absurd sample
    // (TCP burst, clock skew, anything we didn't anticipate) shouldn't be
    // allowed to drag the displayed average to a nonsense number. Realistic
    // sustained speed has its own ceiling from the network and disk.
    let clamped = kilobytes_per_second.min(SPEED_SAMPLE_CLAMP_KBS);

    let bytes_remaining = max.saturating_sub(current_bytes_downloaded); // bytes

    progress.update_window(clamped);
    push_update(&progress, bytes_remaining).await;
}

pub async fn push_update(progress: &ProgressObject, bytes_remaining: usize) {
    let average_speed = progress.rolling.get_average();
    let time_remaining = (bytes_remaining / 1000) / average_speed.max(1);

    update_ui(progress, average_speed, time_remaining).await;
    update_queue(progress).await;
}

async fn update_ui(
    progress_object: &ProgressObject,
    kilobytes_per_second: usize,
    time_remaining: usize,
) {
    match progress_object.progress_type {
        ProgressType::Download => send!(
            progress_object.sender,
            DownloadManagerSignal::UpdateUIDownloadStats(kilobytes_per_second, time_remaining)
        ),
        ProgressType::Disk => (),
    }
}

async fn update_queue(progress: &ProgressObject) {
    send!(progress.sender, DownloadManagerSignal::UpdateUIQueue)
}
