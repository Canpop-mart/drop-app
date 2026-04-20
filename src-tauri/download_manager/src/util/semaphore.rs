use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

pub struct SyncSemaphore {
    inner: Arc<AtomicUsize>
}

impl Default for SyncSemaphore {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncSemaphore {
    pub fn new() -> Self {
        Self { inner: Arc::new(AtomicUsize::new(0)) }
    }

    pub fn acquire(&self) -> SyncSemaphorePermit  {
        self.inner.fetch_add(1, Ordering::AcqRel);
        SyncSemaphorePermit(self.inner.clone())
    }

    pub fn permits(&self) -> usize {
        self.inner.load(Ordering::Acquire)
    }
}

pub struct SyncSemaphorePermit(Arc<AtomicUsize>);

impl Drop for SyncSemaphorePermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
