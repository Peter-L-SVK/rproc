use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Shared, lock-free settings handle. Cloning is cheap (just an Arc bump);
/// the sampler thread keeps one clone and reads it each tick, the UI thread
/// writes to it.
#[derive(Clone)]
pub struct Settings {
    refresh_ms: Arc<AtomicU64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_ms: Arc::new(AtomicU64::new(DEFAULT_REFRESH_MS)),
        }
    }
}

pub const DEFAULT_REFRESH_MS: u64 = 1000;
pub const MIN_REFRESH_MS: u64 = 100;
pub const MAX_REFRESH_MS: u64 = 30_000;

/// Curated presets surfaced as quick-pick buttons in the Settings page.
pub const REFRESH_PRESETS: &[(u64, &str)] = &[
    (250, "4× / s"),
    (500, "2× / s"),
    (1000, "1× / s"),
    (2000, "Every 2 s"),
    (5000, "Every 5 s"),
    (10_000, "Every 10 s"),
];

impl Settings {
    pub fn refresh_ms(&self) -> u64 {
        self.refresh_ms.load(Ordering::Relaxed)
    }

    pub fn set_refresh_ms(&self, ms: u64) {
        self.refresh_ms
            .store(ms.clamp(MIN_REFRESH_MS, MAX_REFRESH_MS), Ordering::Relaxed);
    }

    /// Get the underlying Arc so the sampler thread can read updates
    /// without going through the Settings wrapper.
    pub fn refresh_handle(&self) -> Arc<AtomicU64> {
        self.refresh_ms.clone()
    }
}
