use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn, info};

pub struct MemoryMonitor {
    threshold: f32,
    shutdown_token: CancellationToken,
    cached_usage: Arc<RwLock<Option<(MemoryUsage, Instant)>>>,
    cache_duration: Duration,
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryMonitor {
    pub fn new() -> Self {
        Self {
            threshold: 80.0,
            shutdown_token: CancellationToken::new(),
            cached_usage: Arc::new(RwLock::new(None)),
            cache_duration: Duration::from_secs(5), // Cache for 5 seconds
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<tokio::task::JoinHandle<()>> {
        let mut interval = interval(Duration::from_secs(30));
        let shutdown_token = self.shutdown_token.clone();
        let cached_usage = self.cached_usage.clone();
        let threshold = self.threshold; // FIX: Capture threshold before moving into async block

        let handle = tokio::spawn(async move {
            info!("Memory monitor started");
            let mut sys = System::new();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Perform async memory check
                        sys.refresh_memory();

                        let used = sys.used_memory();
                        let total = sys.total_memory();
                        let available = sys.available_memory();
                        let percentage = if total > 0 {
                            (used as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        };

                        // Update cache
                        let usage = MemoryUsage {
                            used_mb: used as f64 / 1_024_000.0,
                            total_mb: total as f64 / 1_024_000.0,
                            available_mb: available as f64 / 1_024_000.0,
                            percentage,
                        };

                        {
                            let mut cache = cached_usage.write().await;
                            *cache = Some((usage.clone(), Instant::now()));
                        }

                        debug!("Memory usage: {:.1}%", percentage);

                        // FIX: Use captured threshold instead of self.threshold
                        if percentage > threshold as f64 {
                            warn!("High memory usage: {:.1}%", percentage);
                        }
                    }
                    _ = shutdown_token.cancelled() => {
                        info!("Memory monitor shutting down");
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    /// Get cached memory usage if available and fresh
    pub async fn get_cached_usage(&self) -> Option<MemoryUsage> {
        let cache = self.cached_usage.read().await;
        if let Some((usage, timestamp)) = cache.as_ref() {
            if Instant::now().duration_since(*timestamp) < self.cache_duration {
                return Some(usage.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub used_mb: f64,
    pub total_mb: f64,
    pub available_mb: f64,
    pub percentage: f64,
}

/// Type alias for the complex memory cache type
type MemoryCacheType = std::sync::OnceLock<Arc<RwLock<Option<(MemoryUsage, Instant)>>>>;

/// Cached memory usage with rate limiting
static MEMORY_CACHE: MemoryCacheType = std::sync::OnceLock::new();

/// Get current memory usage information with caching to avoid thread pool exhaustion
pub async fn get_memory_usage() -> MemoryUsage {
    let cache = MEMORY_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(None))
    });

    // Check cache first
    {
        let cached = cache.read().await;
        if let Some((usage, timestamp)) = cached.as_ref() {
            // Return cached value if less than 2 seconds old
            if Instant::now().duration_since(*timestamp) < Duration::from_secs(2) {
                return usage.clone();
            }
        }
    }

    // Use a dedicated thread for system info to avoid blocking the runtime
    // But limit concurrent calls with a semaphore
    static MEMORY_SEMAPHORE: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let semaphore = MEMORY_SEMAPHORE.get_or_init(|| {
        tokio::sync::Semaphore::new(1) // Only one memory check at a time
    });

    let permit = match semaphore.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            // If another check is in progress, return cached or default
            let cached = cache.read().await;
            if let Some((usage, _)) = cached.as_ref() {
                return usage.clone();
            }
            return MemoryUsage {
                used_mb: 0.0,
                total_mb: 0.0,
                available_mb: 0.0,
                percentage: 0.0,
            };
        }
    };

    // Perform the actual memory check
    let result = tokio::task::spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_memory();

        let used = sys.used_memory();
        let total = sys.total_memory();
        let available = sys.available_memory();

        let used_mb = used as f64 / 1_024_000.0;
        let total_mb = total as f64 / 1_024_000.0;
        let available_mb = available as f64 / 1_024_000.0;
        let percentage = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        MemoryUsage {
            used_mb,
            total_mb,
            available_mb,
            percentage,
        }
    })
    .await
    .unwrap_or(MemoryUsage {
        used_mb: 0.0,
        total_mb: 0.0,
        available_mb: 0.0,
        percentage: 0.0,
    });

    // Update cache
    {
        let mut cached = cache.write().await;
        *cached = Some((result.clone(), Instant::now()));
    }

    drop(permit); // Explicitly release the semaphore
    result
}
