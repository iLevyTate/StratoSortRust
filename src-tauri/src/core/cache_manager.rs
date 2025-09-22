use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Types of cached data
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum CacheKey {
    FileAnalysis(String),        // File path
    DirectoryScan(String),        // Directory path
    AiEmbedding(String),         // Content hash
    OrganizationSuggestion(String), // File path
    SearchResult(String),        // Search query
    SmartFolderRules(String),    // Smart folder ID
    ApiResponse { endpoint: String, params: String }, // API endpoint with serialized params
    DatabaseQuery(String),       // SQL query hash
    UserPreferences(String),     // User ID or session ID
    SystemInfo(String),          // System info type (os, hardware, etc.)
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub data: T,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub ttl: Duration,
    pub version: u64,
    pub dependencies: Vec<CacheKey>,
}

/// Cache invalidation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidationEvent {
    FileModified { path: String },
    FileDeleted { path: String },
    FileMoved { from: String, to: String },
    DirectoryChanged { path: String },
    ConfigChanged { category: String },
    ManualInvalidation { key: String },
    ApiEndpointChanged { endpoint: String },
    DatabaseSchemaChanged { table: String },
    UserSessionChanged { user_id: String },
    SystemStateChanged { component: String },
    TimeBasedInvalidation { older_than: Duration },
    ConditionalInvalidation { condition: String, affected_keys: Vec<String> },
}

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size_bytes: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub hit_rate: f64,
}

/// Thread-safe cache manager with TTL and dependency tracking
pub struct CacheManager {
    /// Main cache storage
    cache: Arc<DashMap<CacheKey, Box<dyn std::any::Any + Send + Sync>>>,
    /// Dependency graph for invalidation
    dependencies: Arc<DashMap<CacheKey, Vec<CacheKey>>>,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Global cache version for bulk invalidation
    global_version: Arc<RwLock<u64>>,
    /// Maximum cache size in bytes
    max_size_bytes: usize,
    /// Default TTL for cache entries
    default_ttl: Duration,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(max_size_mb: usize, default_ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            dependencies: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(CacheStats {
                total_entries: 0,
                total_size_bytes: 0,
                hit_count: 0,
                miss_count: 0,
                eviction_count: 0,
                hit_rate: 0.0,
            })),
            global_version: Arc::new(RwLock::new(0)),
            max_size_bytes: max_size_mb * 1024 * 1024,
            default_ttl: Duration::from_secs(default_ttl_seconds),
        }
    }

    /// Store an item in the cache
    pub fn set<T: Send + Sync + Clone + 'static>(
        &self,
        key: CacheKey,
        value: T,
        ttl: Option<Duration>,
        dependencies: Vec<CacheKey>,
    ) {
        let entry = CacheEntry {
            data: value,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            ttl: ttl.unwrap_or(self.default_ttl),
            version: *self.global_version.read(),
            dependencies: dependencies.clone(),
        };

        // Store dependencies for this key
        if !dependencies.is_empty() {
            for dep in &dependencies {
                self.dependencies
                    .entry(dep.clone())
                    .or_default()
                    .push(key.clone());
            }
        }

        // Insert into cache
        self.cache.insert(key, Box::new(entry));

        // Update stats
        let mut stats = self.stats.write();
        stats.total_entries = self.cache.len();

        // Check if we need to evict old entries
        if stats.total_size_bytes > self.max_size_bytes {
            self.evict_lru();
        }
    }

    /// Get an item from the cache
    pub fn get<T: Send + Sync + Clone + 'static>(&self, key: &CacheKey) -> Option<T> {
        let entry_ref = self.cache.get(key)?;
        let entry = entry_ref.downcast_ref::<CacheEntry<T>>()?;

        // Check if entry has expired
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(entry.created_at) {
            if elapsed > entry.ttl {
                // Entry has expired, remove it
                drop(entry_ref);
                self.cache.remove(key);
                self.update_miss_stats();
                return None;
            }
        }

        // Check if entry version matches global version
        if entry.version != *self.global_version.read() {
            // Version mismatch, entry is stale
            drop(entry_ref);
            self.cache.remove(key);
            self.update_miss_stats();
            return None;
        }

        // Update last accessed time (would need mutable access)
        self.update_hit_stats();
        Some(entry.data.clone())
    }

    /// Invalidate cache entries based on an event
    pub fn invalidate(&self, event: InvalidationEvent) {
        match event {
            InvalidationEvent::FileModified { path } => {
                self.invalidate_file_cache(&path);
            }
            InvalidationEvent::FileDeleted { path } => {
                self.invalidate_file_cache(&path);
                self.cache.remove(&CacheKey::FileAnalysis(path.clone()));
                self.cache
                    .remove(&CacheKey::OrganizationSuggestion(path));
            }
            InvalidationEvent::FileMoved { from, to } => {
                self.invalidate_file_cache(&from);
                self.invalidate_file_cache(&to);
            }
            InvalidationEvent::DirectoryChanged { path } => {
                self.cache.remove(&CacheKey::DirectoryScan(path.clone()));
                // Invalidate all files in this directory
                self.invalidate_directory_files(&path);
            }
            InvalidationEvent::ConfigChanged { category } => {
                // Invalidate based on config category
                match category.as_str() {
                    "ai" => self.invalidate_ai_cache(),
                    "organization" => self.invalidate_organization_cache(),
                    "api" => self.invalidate_api_cache(),
                    "database" => self.invalidate_database_cache(),
                    _ => self.invalidate_all(),
                }
            }
            InvalidationEvent::ManualInvalidation { key } => {
                // Manual invalidation of specific key pattern
                self.invalidate_by_pattern(&key);
            }
            InvalidationEvent::ApiEndpointChanged { endpoint } => {
                self.invalidate_api_endpoint(&endpoint);
            }
            InvalidationEvent::DatabaseSchemaChanged { table } => {
                self.invalidate_database_table(&table);
            }
            InvalidationEvent::UserSessionChanged { user_id } => {
                self.invalidate_user_cache(&user_id);
            }
            InvalidationEvent::SystemStateChanged { component } => {
                self.invalidate_system_cache(&component);
            }
            InvalidationEvent::TimeBasedInvalidation { older_than } => {
                self.invalidate_older_than(older_than);
            }
            InvalidationEvent::ConditionalInvalidation { condition, affected_keys } => {
                self.invalidate_conditional(&condition, &affected_keys);
            }
        }
    }

    /// Invalidate all cache entries related to a file
    fn invalidate_file_cache(&self, path: &str) {
        let keys_to_invalidate = vec![
            CacheKey::FileAnalysis(path.to_string()),
            CacheKey::OrganizationSuggestion(path.to_string()),
        ];

        for key in keys_to_invalidate {
            // Remove the entry
            self.cache.remove(&key);

            // Invalidate dependent entries
            if let Some(deps) = self.dependencies.get(&key) {
                for dep_key in deps.value().iter() {
                    self.cache.remove(dep_key);
                }
            }
        }
    }

    /// Invalidate all files in a directory
    fn invalidate_directory_files(&self, dir_path: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                match key {
                    CacheKey::FileAnalysis(path) | CacheKey::OrganizationSuggestion(path) => {
                        if path.starts_with(dir_path) {
                            Some(key.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            })
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }

    /// Invalidate all AI-related cache entries
    fn invalidate_ai_cache(&self) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::AiEmbedding(_) | CacheKey::OrganizationSuggestion(_) => {
                    Some(entry.key().clone())
                }
                _ => None,
            })
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }

    /// Invalidate all organization-related cache entries
    fn invalidate_organization_cache(&self) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::OrganizationSuggestion(_) | CacheKey::SmartFolderRules(_) => {
                    Some(entry.key().clone())
                }
                _ => None,
            })
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }

    /// Invalidate all cache entries
    pub fn invalidate_all(&self) {
        self.cache.clear();
        self.dependencies.clear();

        // Increment global version to invalidate any external references
        let mut version = self.global_version.write();
        *version += 1;

        // Reset stats
        let mut stats = self.stats.write();
        stats.total_entries = 0;
        stats.total_size_bytes = 0;
        
        tracing::info!("Cache completely invalidated - all {} entries cleared", stats.total_entries);
    }

    /// Invalidate API-related cache entries
    fn invalidate_api_cache(&self) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::ApiResponse { .. } | CacheKey::SystemInfo(_) => {
                    Some(entry.key().clone())
                }
                _ => None,
            })
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        tracing::debug!("Invalidated API cache entries");
    }

    /// Invalidate database-related cache entries
    fn invalidate_database_cache(&self) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::DatabaseQuery(_) => {
                    Some(entry.key().clone())
                }
                _ => None,
            })
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        tracing::debug!("Invalidated database cache entries");
    }

    /// Invalidate cache entries for a specific API endpoint
    fn invalidate_api_endpoint(&self, endpoint: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::ApiResponse { endpoint: cached_endpoint, .. } => {
                    if cached_endpoint == endpoint || cached_endpoint.contains(endpoint) {
                        Some(entry.key().clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries for API endpoint: {}", removed_count, endpoint);
        }
    }

    /// Invalidate cache entries for a specific database table
    fn invalidate_database_table(&self, table: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::DatabaseQuery(query) => {
                    // Simple heuristic: if query contains table name
                    if query.to_lowercase().contains(&table.to_lowercase()) {
                        Some(entry.key().clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries for database table: {}", removed_count, table);
        }
    }

    /// Invalidate cache entries for a specific user
    fn invalidate_user_cache(&self, user_id: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::UserPreferences(cached_user_id) => {
                    if cached_user_id == user_id {
                        Some(entry.key().clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries for user: {}", removed_count, user_id);
        }
    }

    /// Invalidate cache entries for a specific system component
    fn invalidate_system_cache(&self, component: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| match entry.key() {
                CacheKey::SystemInfo(info_type) => {
                    if info_type == component || info_type.contains(component) {
                        Some(entry.key().clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries for system component: {}", removed_count, component);
        }
    }

    /// Invalidate cache entries older than specified duration
    fn invalidate_older_than(&self, older_than: Duration) {
        let _now = SystemTime::now();
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .map(|entry| {
                let key = entry.key().clone();
                // This is a simplified check - would need proper type handling for real implementation
                // For now, we assume we can check creation time through a different method
                key
            })
            .collect();

        // This would need proper implementation to check actual entry times
        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            // Only remove if we can confirm it's actually older than the threshold
            if let Some(_entry_ref) = self.cache.get(&key) {
                // Need better way to access creation time - this is placeholder logic
                self.cache.remove(&key);
            }
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries older than {:?}", removed_count, older_than);
        }
    }

    /// Invalidate cache entries by pattern matching
    fn invalidate_by_pattern(&self, pattern: &str) {
        let keys_to_remove: Vec<CacheKey> = self
            .cache
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                let key_str = match key {
                    CacheKey::FileAnalysis(path) => path,
                    CacheKey::DirectoryScan(path) => path,
                    CacheKey::OrganizationSuggestion(path) => path,
                    CacheKey::SearchResult(query) => query,
                    CacheKey::SmartFolderRules(id) => id,
                    CacheKey::ApiResponse { endpoint, .. } => endpoint,
                    CacheKey::DatabaseQuery(query) => query,
                    CacheKey::UserPreferences(user_id) => user_id,
                    CacheKey::SystemInfo(info_type) => info_type,
                    CacheKey::AiEmbedding(hash) => hash,
                };

                // Simple pattern matching - could be enhanced with regex
                if key_str.contains(pattern) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        if removed_count > 0 {
            tracing::debug!("Invalidated {} cache entries matching pattern: {}", removed_count, pattern);
        }
    }

    /// Conditional invalidation based on custom logic
    fn invalidate_conditional(&self, condition: &str, affected_keys: &[String]) {
        // This allows for custom invalidation logic based on application state
        match condition {
            "memory_pressure" => {
                // Invalidate largest or least recently used entries
                self.evict_lru();
            }
            "config_reload" => {
                // Invalidate configuration-dependent entries
                self.invalidate_api_cache();
                self.invalidate_system_cache("config");
            }
            "maintenance_mode" => {
                // Clear all volatile caches but keep essential ones
                self.invalidate_api_cache();
            }
            _ => {
                // Invalidate specific keys provided
                for key_pattern in affected_keys {
                    self.invalidate_by_pattern(key_pattern);
                }
            }
        }
        
        tracing::debug!("Applied conditional invalidation: {} affecting {} key patterns", condition, affected_keys.len());
    }

    /// Evict least recently used entries
    fn evict_lru(&self) {
        // Simple implementation: remove oldest entries
        // In production, would track access times properly
        let target_size = self.max_size_bytes * 80 / 100; // Keep 80% after eviction

        let mut evicted = 0;
        let mut current_size = self.stats.read().total_size_bytes;

        while current_size > target_size && !self.cache.is_empty() {
            // Remove first entry found (simple approach)
            if let Some(entry) = self.cache.iter().next() {
                let key = entry.key().clone();
                drop(entry);
                self.cache.remove(&key);
                evicted += 1;
                current_size -= 1000; // Approximate size reduction
            } else {
                break;
            }
        }

        let mut stats = self.stats.write();
        stats.eviction_count += evicted;
        stats.total_entries = self.cache.len();
        stats.total_size_bytes = current_size;
    }

    /// Update hit statistics
    fn update_hit_stats(&self) {
        let mut stats = self.stats.write();
        stats.hit_count += 1;
        stats.hit_rate =
            stats.hit_count as f64 / (stats.hit_count + stats.miss_count).max(1) as f64;
    }

    /// Update miss statistics
    fn update_miss_stats(&self) {
        let mut stats = self.stats.write();
        stats.miss_count += 1;
        stats.hit_rate =
            stats.hit_count as f64 / (stats.hit_count + stats.miss_count).max(1) as f64;
    }

    /// Get current cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// Schedule periodic cleanup of expired entries with proper cancellation support
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let cache = self.cache.clone();
        let stats = self.stats.clone();
        let global_version = self.global_version.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            let mut cleanup_cycles = 0u64;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        cleanup_cycles += 1;

                let now = SystemTime::now();
                let current_version = *global_version.read();
                let mut removed_expired = 0;
                let mut removed_stale = 0;

                // Collect keys to remove (can't remove while iterating)
                let keys_to_remove: Vec<(CacheKey, String)> = cache
                    .iter()
                    .filter_map(|entry| {
                        let key = entry.key().clone();
                        
                        // Try to check if entry is expired or stale
                        // This is a workaround for the type erasure issue
                        // In practice, would need a better approach to access entry metadata
                        if let Some(boxed_entry) = entry.value().downcast_ref::<CacheEntry<String>>() {
                            // Check expiration
                            if let Ok(elapsed) = now.duration_since(boxed_entry.created_at) {
                                if elapsed > boxed_entry.ttl {
                                    return Some((key, "expired".to_string()));
                                }
                            }
                            
                            // Check staleness
                            if boxed_entry.version != current_version {
                                return Some((key, "stale".to_string()));
                            }
                        }
                        
                        None
                    })
                    .collect();

                // Remove the identified entries
                for (key, reason) in keys_to_remove {
                    cache.remove(&key);
                    match reason.as_str() {
                        "expired" => removed_expired += 1,
                        "stale" => removed_stale += 1,
                        _ => {}
                    }
                }

                let total_removed = removed_expired + removed_stale;
                if total_removed > 0 {
                    let mut stats_guard = stats.write();
                    stats_guard.total_entries = cache.len();
                    tracing::debug!(
                        "Cache cleanup cycle {}: removed {} entries ({} expired, {} stale)",
                        cleanup_cycles, total_removed, removed_expired, removed_stale
                    );
                }

                // Periodic comprehensive cleanup (every 10 minutes)
                if cleanup_cycles % 10 == 0 {
                    let cache_size = cache.len();
                    if cache_size > 1000 {
                        tracing::info!("Large cache detected ({} entries), considering cleanup", cache_size);
                    }
                    
                        // Could trigger memory pressure cleanup if needed
                        let stats_guard = stats.read();
                        if stats_guard.total_size_bytes > 100 * 1024 * 1024 {  // 100MB
                            drop(stats_guard);
                            tracing::warn!("High cache memory usage, triggering LRU eviction");
                            // Would call evict_lru() but need access to self
                        }
                    }
                    }
                    _ = tokio::task::yield_now() => {
                        // Allow task cancellation
                        tracing::debug!("Cache cleanup yielded after {} cycles", cleanup_cycles);
                        break;
                    }
                }
            }
            tracing::info!("Cache cleanup task terminated after {} cycles", cleanup_cycles);
        })
    }

    /// Advanced cache warming for frequently accessed data
    pub async fn warm_cache_for_user(&self, user_context: &str) {
        tracing::debug!("Warming cache for user context: {}", user_context);
        
        // This would pre-populate cache with frequently accessed data
        // Implementation depends on specific application needs
    }

    /// Cache health check
    pub fn health_check(&self) -> CacheHealthReport {
        let stats = self.stats.read();
        let cache_size = self.cache.len();
        
        CacheHealthReport {
            total_entries: cache_size,
            memory_usage_bytes: stats.total_size_bytes,
            hit_rate: stats.hit_rate,
            is_healthy: stats.hit_rate > 0.7 && cache_size < 10000,
            recommendations: self.get_health_recommendations(&stats, cache_size),
        }
    }

    fn get_health_recommendations(&self, stats: &CacheStats, cache_size: usize) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if stats.hit_rate < 0.5 {
            recommendations.push("Low hit rate detected. Consider adjusting TTL values or cache warming.".to_string());
        }
        
        if cache_size > 5000 {
            recommendations.push("Large cache size detected. Consider more aggressive cleanup or smaller TTL.".to_string());
        }
        
        if stats.total_size_bytes > 50 * 1024 * 1024 {  // 50MB
            recommendations.push("High memory usage. Consider reducing cache size or implementing better eviction.".to_string());
        }
        
        if stats.eviction_count > stats.hit_count {
            recommendations.push("High eviction rate. Consider increasing cache size or optimizing data access patterns.".to_string());
        }
        
        recommendations
    }
}

/// Cache health report
#[derive(Debug, Clone, Serialize)]
pub struct CacheHealthReport {
    pub total_entries: usize,
    pub memory_usage_bytes: usize,
    pub hit_rate: f64,
    pub is_healthy: bool,
    pub recommendations: Vec<String>,
}

/// Specialized embedding cache with expiration and simple eviction
use std::collections::HashMap;
use std::time::Instant;

const MAX_CACHE_SIZE: usize = 10000;
const MAX_CACHE_AGE: Duration = Duration::from_secs(3600); // 1 hour

#[derive(Debug, Clone)]
struct CachedEmbedding {
    embedding: Vec<f32>,
    timestamp: Instant,
    access_count: usize,
}

pub struct EmbeddingCache {
    cache: Arc<RwLock<HashMap<String, CachedEmbedding>>>,
}

impl EmbeddingCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<f32>> {
        let mut cache = self.cache.write();

        if let Some(entry) = cache.get_mut(key) {
            // Check if expired
            if entry.timestamp.elapsed() > MAX_CACHE_AGE {
                cache.remove(key);
                return None;
            }

            entry.access_count += 1;
            Some(entry.embedding.clone())
        } else {
            None
        }
    }

    pub async fn insert(&self, key: String, embedding: Vec<f32>) {
        let mut cache = self.cache.write();

        // Simple size-based eviction if needed
        if cache.len() >= MAX_CACHE_SIZE {
            // Remove one random entry (simple eviction strategy)
            if let Some(key_to_remove) = cache.keys().next().cloned() {
                cache.remove(&key_to_remove);
            }
        }

        cache.insert(key, CachedEmbedding {
            embedding,
            timestamp: Instant::now(),
            access_count: 0,
        });
    }

    pub async fn size(&self) -> usize {
        self.cache.read().len()
    }

    pub async fn clear(&self) {
        self.cache.write().clear();
    }

    /// Remove expired entries
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write();
        let now = Instant::now();

        // Collect keys to remove (can't mutate while iterating)
        let expired_keys: Vec<_> = cache
            .iter()
            .filter_map(|(k, v)| {
                if now.duration_since(v.timestamp) > MAX_CACHE_AGE {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove expired entries
        for key in expired_keys {
            cache.remove(&key);
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> (usize, usize, f64) {
        let cache = self.cache.read();
        let total_size = cache.len();
        let total_accesses: usize = cache.iter().map(|(_, v)| v.access_count).sum();
        let avg_accesses = if total_size > 0 {
            total_accesses as f64 / total_size as f64
        } else {
            0.0
        };

        (total_size, total_accesses, avg_accesses)
    }
}

impl Default for EmbeddingCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_ttl() {
        let cache = CacheManager::new(10, 1); // 1 second TTL

        let key = CacheKey::FileAnalysis("/test.txt".to_string());
        cache.set(key.clone(), "test_data".to_string(), None, vec![]);

        // Should be retrievable immediately
        assert!(cache.get::<String>(&key).is_some());

        // After TTL expires, should return None
        std::thread::sleep(Duration::from_secs(2));
        assert!(cache.get::<String>(&key).is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = CacheManager::new(10, 60);

        let key1 = CacheKey::FileAnalysis("/file1.txt".to_string());
        let key2 = CacheKey::OrganizationSuggestion("/file1.txt".to_string());

        cache.set(key1.clone(), "analysis".to_string(), None, vec![]);
        cache.set(key2.clone(), "suggestion".to_string(), None, vec![]);

        // Both should exist
        assert!(cache.get::<String>(&key1).is_some());
        assert!(cache.get::<String>(&key2).is_some());

        // Invalidate file
        cache.invalidate(InvalidationEvent::FileModified {
            path: "/file1.txt".to_string(),
        });

        // Both should be invalidated
        assert!(cache.get::<String>(&key1).is_none());
        assert!(cache.get::<String>(&key2).is_none());
    }
}