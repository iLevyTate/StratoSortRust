// Advanced Caching System with Multi-Layer Strategy
// Provides intelligent caching with automatic invalidation and warming

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use async_trait::async_trait;

use crate::error::AppError;

// Cache entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub key: String,
    pub value: T,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
    pub size_bytes: usize,
    pub tags: HashSet<String>,
    pub priority: CachePriority,
}

// Cache priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CachePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

// Cache eviction policy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LRU,  // Least Recently Used
    LFU,  // Least Frequently Used
    FIFO, // First In First Out
    TTL,  // Time To Live based
    ARC,  // Adaptive Replacement Cache
}

// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expirations: u64,
    pub total_size_bytes: usize,
    pub entry_count: usize,
    pub hit_rate: f64,
    pub avg_access_time_ms: f64,
    pub memory_usage_bytes: usize,
}

// Cache layer trait
#[async_trait]
pub trait CacheLayer: Send + Sync {
    async fn get(&self, key: &str) -> Option<Vec<u8>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), AppError>;
    async fn delete(&self, key: &str) -> Result<bool, AppError>;
    async fn clear(&self) -> Result<(), AppError>;
    async fn exists(&self, key: &str) -> bool;
    async fn size(&self) -> usize;
    fn name(&self) -> &str;
}

// In-memory cache layer (L1)
pub struct MemoryCache {
    data: Arc<RwLock<HashMap<String, CacheEntry<Vec<u8>>>>>,
    max_size: usize,
    eviction_policy: EvictionPolicy,
    stats: Arc<RwLock<CacheStats>>,
}

impl MemoryCache {
    pub fn new(max_size: usize, eviction_policy: EvictionPolicy) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            eviction_policy,
            stats: Arc::new(RwLock::new(CacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
                expirations: 0,
                total_size_bytes: 0,
                entry_count: 0,
                hit_rate: 0.0,
                avg_access_time_ms: 0.0,
                memory_usage_bytes: 0,
            })),
        }
    }

    async fn evict_if_needed(&self) {
        let mut data = self.data.write().await;

        while self.calculate_total_size(&data) > self.max_size && !data.is_empty() {
            let key_to_evict = match self.eviction_policy {
                EvictionPolicy::LRU => {
                    // Find least recently used
                    data.iter()
                        .min_by_key(|(_, entry)| entry.last_accessed)
                        .map(|(k, _)| k.clone())
                }
                EvictionPolicy::LFU => {
                    // Find least frequently used
                    data.iter()
                        .min_by_key(|(_, entry)| entry.access_count)
                        .map(|(k, _)| k.clone())
                }
                EvictionPolicy::FIFO => {
                    // Find oldest entry
                    data.iter()
                        .min_by_key(|(_, entry)| entry.created_at)
                        .map(|(k, _)| k.clone())
                }
                EvictionPolicy::TTL => {
                    // Find entry closest to expiration
                    data.iter()
                        .filter_map(|(k, entry)| entry.expires_at.map(|exp| (k, exp)))
                        .min_by_key(|(_, exp)| *exp)
                        .map(|(k, _)| k.clone())
                }
                EvictionPolicy::ARC => {
                    // Adaptive replacement - combine LRU and LFU
                    data.iter()
                        .min_by_key(|(_, entry)| {
                            let recency_score = Utc::now().timestamp() - entry.last_accessed.timestamp();
                            let frequency_score = 1.0 / (entry.access_count as f64 + 1.0);
                            (recency_score as f64 * frequency_score * 1000.0) as i64
                        })
                        .map(|(k, _)| k.clone())
                }
            };

            if let Some(key) = key_to_evict {
                data.remove(&key);
                let mut stats = self.stats.write().await;
                stats.evictions += 1;
            } else {
                break;
            }
        }
    }

    fn calculate_total_size(&self, data: &HashMap<String, CacheEntry<Vec<u8>>>) -> usize {
        data.values().map(|entry| entry.size_bytes).sum()
    }

    async fn clean_expired(&self) {
        let mut data = self.data.write().await;
        let now = Utc::now();
        let expired_keys: Vec<String> = data
            .iter()
            .filter(|(_, entry)| {
                entry.expires_at.is_some_and(|exp| exp < now)
            })
            .map(|(k, _)| k.clone())
            .collect();

        let mut stats = self.stats.write().await;
        for key in expired_keys {
            data.remove(&key);
            stats.expirations += 1;
        }
    }
}

#[async_trait]
impl CacheLayer for MemoryCache {
    async fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.clean_expired().await;

        let mut data = self.data.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = data.get_mut(key) {
            entry.last_accessed = Utc::now();
            entry.access_count += 1;
            stats.hits += 1;
            stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;
            Some(entry.value.clone())
        } else {
            stats.misses += 1;
            stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;
            None
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), AppError> {
        let expires_at = ttl.and_then(|d| chrono::Duration::from_std(d).ok().map(|cd| Utc::now() + cd));

        let entry = CacheEntry {
            key: key.to_string(),
            value: value.clone(),
            created_at: Utc::now(),
            expires_at,
            last_accessed: Utc::now(),
            access_count: 0,
            size_bytes: value.len(),
            tags: HashSet::new(),
            priority: CachePriority::Normal,
        };

        {
            let mut data = self.data.write().await;
            data.insert(key.to_string(), entry);
        }

        self.evict_if_needed().await;

        let mut stats = self.stats.write().await;
        stats.entry_count = self.data.read().await.len();
        stats.total_size_bytes = self.calculate_total_size(&*self.data.read().await);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, AppError> {
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }

    async fn clear(&self) -> Result<(), AppError> {
        let mut data = self.data.write().await;
        data.clear();

        let mut stats = self.stats.write().await;
        *stats = CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            expirations: 0,
            total_size_bytes: 0,
            entry_count: 0,
            hit_rate: 0.0,
            avg_access_time_ms: 0.0,
            memory_usage_bytes: 0,
        };

        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        self.data.read().await.contains_key(key)
    }

    async fn size(&self) -> usize {
        self.calculate_total_size(&*self.data.read().await)
    }

    fn name(&self) -> &str {
        "memory"
    }
}

// Disk cache layer (L2)
pub struct DiskCache {
    cache_dir: PathBuf,
    #[allow(dead_code)]
    max_size: usize,
    index: Arc<RwLock<HashMap<String, CacheEntry<PathBuf>>>>,
}

impl DiskCache {
    pub async fn new(cache_dir: PathBuf, max_size: usize) -> Result<Self, AppError> {
        // Ensure cache directory exists
        tokio::fs::create_dir_all(&cache_dir).await
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to create cache directory: {}", e),
            })?;

        Ok(Self {
            cache_dir,
            max_size,
            index: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn get_file_path(&self, key: &str) -> PathBuf {
        // Note: Would use sha2 for secure hashing in production
        // For now, using simple hash based on std library
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        self.cache_dir.join(format!("{:x}.cache", hash))
    }
}

#[async_trait]
impl CacheLayer for DiskCache {
    async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let file_path = self.get_file_path(key);

        if let Ok(data) = tokio::fs::read(&file_path).await {
            let mut index = self.index.write().await;
            if let Some(entry) = index.get_mut(key) {
                entry.last_accessed = Utc::now();
                entry.access_count += 1;
            }
            Some(data)
        } else {
            None
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), AppError> {
        let file_path = self.get_file_path(key);

        // Write to disk
        tokio::fs::write(&file_path, &value).await
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to write cache file: {}", e),
            })?;

        // Update index
        let expires_at = ttl.and_then(|d| chrono::Duration::from_std(d).ok().map(|cd| Utc::now() + cd));

        let entry = CacheEntry {
            key: key.to_string(),
            value: file_path.clone(),
            created_at: Utc::now(),
            expires_at,
            last_accessed: Utc::now(),
            access_count: 0,
            size_bytes: value.len(),
            tags: HashSet::new(),
            priority: CachePriority::Normal,
        };

        let mut index = self.index.write().await;
        index.insert(key.to_string(), entry);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, AppError> {
        let file_path = self.get_file_path(key);

        let exists = tokio::fs::remove_file(&file_path).await.is_ok();

        let mut index = self.index.write().await;
        index.remove(key);

        Ok(exists)
    }

    async fn clear(&self) -> Result<(), AppError> {
        // Clear all cache files
        if let Ok(mut entries) = tokio::fs::read_dir(&self.cache_dir).await {
            while let Some(entry) = entries.next_entry().await.ok().flatten() {
                if entry.file_name().to_str().is_some_and(|n| n.ends_with(".cache")) {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }

        let mut index = self.index.write().await;
        index.clear();

        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        let file_path = self.get_file_path(key);
        tokio::fs::metadata(&file_path).await.is_ok()
    }

    async fn size(&self) -> usize {
        let index = self.index.read().await;
        index.values().map(|e| e.size_bytes).sum()
    }

    fn name(&self) -> &str {
        "disk"
    }
}

// Multi-layer cache manager
pub struct AdvancedCache {
    layers: Vec<Arc<dyn CacheLayer>>,
    warming_queue: Arc<Mutex<VecDeque<String>>>,
    invalidation_rules: Arc<RwLock<HashMap<String, InvalidationRule>>>,
    stats: Arc<RwLock<CacheStats>>,
    config: CacheConfig,
}

// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub enable_warming: bool,
    pub warming_batch_size: usize,
    pub enable_compression: bool,
    pub compression_threshold: usize,
    pub enable_encryption: bool,
    pub default_ttl: Duration,
    pub max_key_length: usize,
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enable_warming: true,
            warming_batch_size: 10,
            enable_compression: true,
            compression_threshold: 1024, // Compress if > 1KB
            enable_encryption: false,
            default_ttl: Duration::from_secs(3600), // 1 hour
            max_key_length: 250,
            enable_metrics: true,
        }
    }
}

// Invalidation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationRule {
    pub pattern: String,
    pub rule_type: InvalidationType,
    pub dependencies: Vec<String>,
    pub ttl_override: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidationType {
    TagBased(Vec<String>),
    PatternBased(String),
    TimeBased(Duration),
    DependencyBased(Vec<String>),
    EventBased(String),
}

impl AdvancedCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            layers: Vec::new(),
            warming_queue: Arc::new(Mutex::new(VecDeque::new())),
            invalidation_rules: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
                expirations: 0,
                total_size_bytes: 0,
                entry_count: 0,
                hit_rate: 0.0,
                avg_access_time_ms: 0.0,
                memory_usage_bytes: 0,
            })),
            config,
        }
    }

    // Add a cache layer
    pub fn add_layer(&mut self, layer: Arc<dyn CacheLayer>) {
        self.layers.push(layer);
    }

    // Get value with multi-layer lookup
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let start = Instant::now();

        for (index, layer) in self.layers.iter().enumerate() {
            if let Some(data) = layer.get(key).await {
                // Update stats
                let mut stats = self.stats.write().await;
                stats.hits += 1;
                stats.avg_access_time_ms =
                    (stats.avg_access_time_ms * (stats.hits - 1) as f64 + start.elapsed().as_millis() as f64)
                    / stats.hits as f64;

                // Promote to higher layers
                if index > 0 {
                    for i in (0..index).rev() {
                        let _ = self.layers[i].set(key, data.clone(), Some(self.config.default_ttl)).await;
                    }
                }

                // Deserialize
                if let Ok(value) = serde_json::from_slice(&data) {
                    return Some(value);
                }
            }
        }

        // Cache miss
        let mut stats = self.stats.write().await;
        stats.misses += 1;
        stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;

        // Add to warming queue if enabled
        if self.config.enable_warming {
            let mut queue = self.warming_queue.lock().await;
            if queue.len() < 1000 { // Limit queue size
                queue.push_back(key.to_string());
            }
        }

        None
    }

    // Set value in all layers
    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), AppError> {
        // Serialize value
        let data = serde_json::to_vec(value).map_err(|e| AppError::SystemError {
            message: format!("Failed to serialize cache value: {}", e),
        })?;

        // Compress if needed
        let final_data = if self.config.enable_compression && data.len() > self.config.compression_threshold {
            self.compress(&data)?
        } else {
            data
        };

        // Set in all layers
        for layer in &self.layers {
            layer.set(key, final_data.clone(), ttl.or(Some(self.config.default_ttl))).await?;
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.entry_count += 1;
        stats.total_size_bytes += final_data.len();

        Ok(())
    }

    // Invalidate cache entry
    pub async fn invalidate(&self, key: &str) -> Result<(), AppError> {
        for layer in &self.layers {
            layer.delete(key).await?;
        }
        Ok(())
    }

    // Invalidate by pattern
    pub async fn invalidate_pattern(&self, _pattern: &str) -> Result<(), AppError> {
        // This would need to be implemented with actual pattern matching
        // For now, just a placeholder
        Ok(())
    }

    // Invalidate by tag
    pub async fn invalidate_tag(&self, _tag: &str) -> Result<(), AppError> {
        // This would need tag tracking implementation
        Ok(())
    }

    // Warm cache with frequently accessed keys
    pub async fn warm_cache<F>(&self, loader: F) -> Result<(), AppError>
    where
        F: Fn(&str) -> Option<Vec<u8>> + Send + Sync,
    {
        let mut queue = self.warming_queue.lock().await;
        // Get the length before creating drain iterator to avoid borrow checker conflict
        let queue_len = queue.len();
        let drain_count = self.config.warming_batch_size.min(queue_len);
        let batch: Vec<String> = queue
            .drain(..drain_count)
            .collect();

        for key in batch {
            if let Some(data) = loader(&key) {
                for layer in &self.layers {
                    let _ = layer.set(&key, data.clone(), Some(self.config.default_ttl)).await;
                }
            }
        }

        Ok(())
    }

    // Clear all cache layers
    pub async fn clear_all(&self) -> Result<(), AppError> {
        for layer in &self.layers {
            layer.clear().await?;
        }

        let mut stats = self.stats.write().await;
        *stats = CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            expirations: 0,
            total_size_bytes: 0,
            entry_count: 0,
            hit_rate: 0.0,
            avg_access_time_ms: 0.0,
            memory_usage_bytes: 0,
        };

        Ok(())
    }

    // Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    // Compress data - placeholder implementation
    // Note: Would require flate2 crate for actual compression
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, AppError> {
        // For now, return data as-is without compression
        // In production, would use flate2 or similar compression library
        Ok(data.to_vec())
    }

    // Decompress data - placeholder implementation
    // Note: Would require flate2 crate for actual decompression
    #[allow(dead_code)]
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, AppError> {
        // For now, return data as-is since we're not compressing
        // In production, would use flate2 or similar compression library
        Ok(data.to_vec())
    }

    // Add invalidation rule
    pub async fn add_invalidation_rule(&self, name: &str, rule: InvalidationRule) {
        let mut rules = self.invalidation_rules.write().await;
        rules.insert(name.to_string(), rule);
    }

    // Process invalidation rules
    pub async fn process_invalidation_rules(&self) {
        let rules = self.invalidation_rules.read().await;

        for (_name, rule) in rules.iter() {
            match &rule.rule_type {
                InvalidationType::TimeBased(_duration) => {
                    // Invalidate entries older than duration
                    // Implementation needed
                }
                InvalidationType::TagBased(tags) => {
                    // Invalidate entries with matching tags
                    for tag in tags {
                        let _ = self.invalidate_tag(tag).await;
                    }
                }
                InvalidationType::PatternBased(pattern) => {
                    // Invalidate entries matching pattern
                    let _ = self.invalidate_pattern(pattern).await;
                }
                InvalidationType::DependencyBased(deps) => {
                    // Invalidate dependent entries
                    for dep in deps {
                        let _ = self.invalidate(dep).await;
                    }
                }
                InvalidationType::EventBased(_event) => {
                    // Handle event-based invalidation
                    // Implementation needed
                }
            }
        }
    }
}

// Type alias for complex key generator type
type KeyGenerator = Box<dyn Fn(&[&str]) -> String + Send + Sync>;

// Cache-aware decorator for functions
pub struct CacheDecorator<F> {
    cache: Arc<AdvancedCache>,
    function: F,
    key_generator: KeyGenerator,
    ttl: Duration,
}

impl<F, T> CacheDecorator<F>
where
    F: Fn() -> T + Send + Sync,
    T: Serialize + serde::de::DeserializeOwned + Send + Sync,
{
    pub fn new(cache: Arc<AdvancedCache>, function: F, ttl: Duration) -> Self {
        Self {
            cache,
            function,
            key_generator: Box::new(|args| args.join(":")),
            ttl,
        }
    }

    pub async fn call(&self, args: &[&str]) -> Result<T, AppError> {
        let key = (self.key_generator)(args);

        // Try cache first
        if let Some(value) = self.cache.get::<T>(&key).await {
            return Ok(value);
        }

        // Execute function
        let result = (self.function)();

        // Cache result
        self.cache.set(&key, &result, Some(self.ttl)).await?;

        Ok(result)
    }
}

// Note: Types are already public, no need for re-export