// Performance Monitoring and Profiling System
// Provides comprehensive performance tracking and analysis

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::AppError;

// Performance metric types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Timer,
    Rate,
}

// Performance metric entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub tags: HashMap<String, String>,
    pub metadata: Option<serde_json::Value>,
}

// Performance snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage: f32,
    pub memory_usage: MemoryUsage,
    pub disk_io: DiskIO,
    pub network_io: NetworkIO,
    pub database_metrics: DatabaseMetrics,
    pub cache_metrics: CacheMetrics,
    pub api_metrics: ApiMetrics,
    pub custom_metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub heap_allocated: u64,
    pub heap_used: u64,
    pub gc_collections: u32,
    pub gc_pause_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIO {
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub read_ops_per_sec: u32,
    pub write_ops_per_sec: u32,
    pub queue_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIO {
    pub bytes_sent_per_sec: u64,
    pub bytes_received_per_sec: u64,
    pub packets_sent_per_sec: u32,
    pub packets_received_per_sec: u32,
    pub active_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetrics {
    pub queries_per_sec: f64,
    pub avg_query_time_ms: f64,
    pub slow_queries: u32,
    pub connection_pool_used: u32,
    pub connection_pool_available: u32,
    pub transaction_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetrics {
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub evictions_per_sec: f64,
    pub size_bytes: u64,
    pub entry_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMetrics {
    pub requests_per_sec: f64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub error_rate: f64,
    pub status_codes: HashMap<u16, u32>,
}

// Performance monitor
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<HashMap<String, Vec<MetricEntry>>>>,
    snapshots: Arc<RwLock<VecDeque<PerformanceSnapshot>>>,
    timers: Arc<RwLock<HashMap<String, Instant>>>,
    counters: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    config: MonitorConfig,
    is_profiling: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub enabled: bool,
    pub snapshot_interval: Duration,
    pub max_snapshots: usize,
    pub max_metric_entries: usize,
    pub profile_sampling_rate: f32,
    pub slow_query_threshold_ms: u64,
    pub export_format: ExportFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Prometheus,
    Graphite,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            snapshot_interval: Duration::from_secs(60),
            max_snapshots: 1440, // 24 hours at 1-minute intervals
            max_metric_entries: 10000,
            profile_sampling_rate: 0.01, // 1% sampling
            slow_query_threshold_ms: 1000,
            export_format: ExportFormat::Json,
        }
    }
}

impl PerformanceMonitor {
    // Create new performance monitor
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(VecDeque::new())),
            timers: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            config,
            is_profiling: Arc::new(RwLock::new(false)),
        }
    }

    // Start profiling session
    pub async fn start_profiling(&self) -> Result<(), AppError> {
        let mut profiling = self.is_profiling.write().await;
        if *profiling {
            return Err(AppError::SystemError {
                message: "Profiling already in progress".to_string(),
            });
        }

        *profiling = true;

        // Start background monitoring task
        let monitor = self.clone();
        tokio::spawn(async move {
            monitor.monitoring_loop().await;
        });

        Ok(())
    }

    // Stop profiling session
    pub async fn stop_profiling(&self) -> Result<ProfilingReport, AppError> {
        let mut profiling = self.is_profiling.write().await;
        if !*profiling {
            return Err(AppError::SystemError {
                message: "No profiling session in progress".to_string(),
            });
        }

        *profiling = false;

        // Generate profiling report
        self.generate_report().await
    }

    // Record a metric
    pub async fn record_metric(&self, metric: MetricEntry) {
        if !self.config.enabled {
            return;
        }

        let mut metrics = self.metrics.write().await;
        let entries = metrics.entry(metric.name.clone()).or_insert_with(Vec::new);

        // Maintain max entries limit
        if entries.len() >= self.config.max_metric_entries {
            entries.remove(0);
        }

        entries.push(metric);
    }

    // Start a timer
    pub async fn start_timer(&self, name: &str) {
        let mut timers = self.timers.write().await;
        timers.insert(name.to_string(), Instant::now());
    }

    // Stop a timer and record duration
    pub async fn stop_timer(&self, name: &str) -> Option<Duration> {
        let mut timers = self.timers.write().await;
        if let Some(start) = timers.remove(name) {
            let duration = start.elapsed();

            // Record as metric
            self.record_metric(MetricEntry {
                name: name.to_string(),
                metric_type: MetricType::Timer,
                value: duration.as_millis() as f64,
                timestamp: Utc::now(),
                tags: HashMap::new(),
                metadata: None,
            }).await;

            Some(duration)
        } else {
            None
        }
    }

    // Increment a counter
    pub async fn increment_counter(&self, name: &str, value: f64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0.0) += value;

        // Record as metric
        self.record_metric(MetricEntry {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value: counters[name],
            timestamp: Utc::now(),
            tags: HashMap::new(),
            metadata: None,
        }).await;
    }

    // Record a value in histogram
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        let values = histograms.entry(name.to_string()).or_insert_with(Vec::new);
        values.push(value);

        // Calculate percentiles
        if values.len() >= 100 {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p50 = values[values.len() / 2];
            let p95 = values[values.len() * 95 / 100];
            let p99 = values[values.len() * 99 / 100];

            // Record percentiles as metrics
            self.record_metric(MetricEntry {
                name: format!("{}.p50", name),
                metric_type: MetricType::Gauge,
                value: p50,
                timestamp: Utc::now(),
                tags: HashMap::new(),
                metadata: None,
            }).await;

            self.record_metric(MetricEntry {
                name: format!("{}.p95", name),
                metric_type: MetricType::Gauge,
                value: p95,
                timestamp: Utc::now(),
                tags: HashMap::new(),
                metadata: None,
            }).await;

            self.record_metric(MetricEntry {
                name: format!("{}.p99", name),
                metric_type: MetricType::Gauge,
                value: p99,
                timestamp: Utc::now(),
                tags: HashMap::new(),
                metadata: None,
            }).await;

            // Keep only recent values
            values.drain(0..values.len() / 2);
        }
    }

    // Take a performance snapshot
    pub async fn take_snapshot(&self) -> Result<PerformanceSnapshot, AppError> {
        // Note: sysinfo 0.32+ removed traits, methods are now directly on System
        use sysinfo::System;

        let mut system = System::new_all();
        system.refresh_all();

        // Get CPU usage
        // In sysinfo 0.32+, use cpus() to get CPU info
        let cpu_usage = system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / system.cpus().len() as f32;

        // Get memory usage
        let memory_usage = MemoryUsage {
            total_bytes: system.total_memory() * 1024,
            used_bytes: system.used_memory() * 1024,
            available_bytes: system.available_memory() * 1024,
            heap_allocated: 0, // Would need platform-specific implementation
            heap_used: 0,
            gc_collections: 0,
            gc_pause_ms: 0,
        };

        // Get disk I/O (simplified)
        let disk_io = DiskIO {
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            read_ops_per_sec: 0,
            write_ops_per_sec: 0,
            queue_depth: 0,
        };

        // Get network I/O
        let network_io = NetworkIO {
            bytes_sent_per_sec: 0,
            bytes_received_per_sec: 0,
            packets_sent_per_sec: 0,
            packets_received_per_sec: 0,
            active_connections: 0,
        };

        // Get database metrics from counters
        let counters = self.counters.read().await;
        let database_metrics = DatabaseMetrics {
            queries_per_sec: *counters.get("db.queries_per_sec").unwrap_or(&0.0),
            avg_query_time_ms: *counters.get("db.avg_query_ms").unwrap_or(&0.0),
            slow_queries: *counters.get("db.slow_queries").unwrap_or(&0.0) as u32,
            connection_pool_used: *counters.get("db.pool_used").unwrap_or(&0.0) as u32,
            connection_pool_available: *counters.get("db.pool_available").unwrap_or(&0.0) as u32,
            transaction_rate: *counters.get("db.transaction_rate").unwrap_or(&0.0),
        };

        // Get cache metrics
        let cache_metrics = CacheMetrics {
            hit_rate: *counters.get("cache.hit_rate").unwrap_or(&0.0),
            miss_rate: *counters.get("cache.miss_rate").unwrap_or(&0.0),
            evictions_per_sec: *counters.get("cache.evictions_per_sec").unwrap_or(&0.0),
            size_bytes: *counters.get("cache.size_bytes").unwrap_or(&0.0) as u64,
            entry_count: *counters.get("cache.entry_count").unwrap_or(&0.0) as u64,
        };

        // Get API metrics
        let api_metrics = ApiMetrics {
            requests_per_sec: *counters.get("api.requests_per_sec").unwrap_or(&0.0),
            avg_response_time_ms: *counters.get("api.avg_response_ms").unwrap_or(&0.0),
            p95_response_time_ms: *counters.get("api.p95_response_ms").unwrap_or(&0.0),
            p99_response_time_ms: *counters.get("api.p99_response_ms").unwrap_or(&0.0),
            error_rate: *counters.get("api.error_rate").unwrap_or(&0.0),
            status_codes: HashMap::new(),
        };

        // Collect custom metrics
        let custom_metrics: HashMap<String, f64> = counters
            .iter()
            .filter(|(k, _)| !k.starts_with("db.") && !k.starts_with("cache.") && !k.starts_with("api."))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            cpu_usage,
            memory_usage,
            disk_io,
            network_io,
            database_metrics,
            cache_metrics,
            api_metrics,
            custom_metrics,
        };

        // Store snapshot
        let mut snapshots = self.snapshots.write().await;
        if snapshots.len() >= self.config.max_snapshots {
            snapshots.pop_front();
        }
        snapshots.push_back(snapshot.clone());

        Ok(snapshot)
    }

    // Monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = tokio::time::interval(self.config.snapshot_interval);

        loop {
            interval.tick().await;

            let profiling = self.is_profiling.read().await;
            if !*profiling {
                break;
            }

            if let Err(e) = self.take_snapshot().await {
                eprintln!("Failed to take performance snapshot: {}", e);
            }
        }
    }

    // Generate profiling report
    pub async fn generate_report(&self) -> Result<ProfilingReport, AppError> {
        let snapshots = self.snapshots.read().await;
        let metrics = self.metrics.read().await;

        // Calculate statistics
        let mut cpu_samples = Vec::new();
        let mut memory_samples = Vec::new();
        let mut response_times = Vec::new();

        for snapshot in snapshots.iter() {
            cpu_samples.push(snapshot.cpu_usage as f64);
            memory_samples.push(snapshot.memory_usage.used_bytes as f64);
            response_times.push(snapshot.api_metrics.avg_response_time_ms);
        }

        let report = ProfilingReport {
            start_time: snapshots.front().map(|s| s.timestamp),
            end_time: snapshots.back().map(|s| s.timestamp),
            duration_seconds: if let (Some(first), Some(last)) = (snapshots.front(), snapshots.back()) {
                (last.timestamp - first.timestamp).num_seconds() as u64
            } else {
                0
            },
            snapshot_count: snapshots.len(),
            cpu_stats: calculate_stats(&cpu_samples),
            memory_stats: calculate_stats(&memory_samples),
            response_time_stats: calculate_stats(&response_times),
            slow_operations: self.get_slow_operations(&metrics),
            hot_paths: self.get_hot_paths(&metrics),
            memory_leaks: self.detect_memory_leaks(&snapshots),
            performance_bottlenecks: self.detect_bottlenecks(&snapshots),
        };

        Ok(report)
    }

    // Get slow operations
    fn get_slow_operations(&self, metrics: &HashMap<String, Vec<MetricEntry>>) -> Vec<SlowOperation> {
        let mut operations = Vec::new();

        for (name, entries) in metrics.iter() {
            if let Some(entry) = entries.iter()
                .filter(|e| matches!(e.metric_type, MetricType::Timer))
                .filter(|e| e.value > self.config.slow_query_threshold_ms as f64)
                .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
            {
                operations.push(SlowOperation {
                    name: name.clone(),
                    duration_ms: entry.value,
                    timestamp: entry.timestamp,
                    tags: entry.tags.clone(),
                });
            }
        }

        operations.sort_by(|a, b| b.duration_ms.partial_cmp(&a.duration_ms).unwrap_or(std::cmp::Ordering::Equal));
        operations.truncate(10); // Top 10 slowest
        operations
    }

    // Get hot paths (most frequently called)
    fn get_hot_paths(&self, metrics: &HashMap<String, Vec<MetricEntry>>) -> Vec<HotPath> {
        let mut paths = Vec::new();

        for (name, entries) in metrics.iter() {
            let count = entries.len();
            let total_time: f64 = entries.iter()
                .filter(|e| matches!(e.metric_type, MetricType::Timer))
                .map(|e| e.value)
                .sum();

            if count > 0 {
                paths.push(HotPath {
                    name: name.clone(),
                    call_count: count,
                    total_time_ms: total_time,
                    avg_time_ms: total_time / count as f64,
                });
            }
        }

        paths.sort_by(|a, b| b.total_time_ms.partial_cmp(&a.total_time_ms).unwrap_or(std::cmp::Ordering::Equal));
        paths.truncate(10); // Top 10 hot paths
        paths
    }

    // Detect potential memory leaks
    fn detect_memory_leaks(&self, snapshots: &VecDeque<PerformanceSnapshot>) -> Vec<MemoryLeak> {
        let mut leaks = Vec::new();

        if snapshots.len() < 10 {
            return leaks;
        }

        // Simple linear regression to detect increasing memory trend
        let memory_values: Vec<f64> = snapshots.iter()
            .map(|s| s.memory_usage.used_bytes as f64)
            .collect();

        if let Some(slope) = calculate_trend(&memory_values) {
            // If memory is increasing by more than 1MB per minute
            if slope > 1_048_576.0 {
                leaks.push(MemoryLeak {
                    growth_rate_bytes_per_sec: slope as u64 / 60,
                    duration_seconds: snapshots.len() as u64 * 60,
                    total_growth_bytes: (slope * snapshots.len() as f64) as u64,
                });
            }
        }

        leaks
    }

    // Detect performance bottlenecks
    fn detect_bottlenecks(&self, snapshots: &VecDeque<PerformanceSnapshot>) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();

        if let Some(last) = snapshots.back() {
            // High CPU usage
            if last.cpu_usage > 80.0 {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::Cpu,
                    severity: if last.cpu_usage > 95.0 { "critical" } else { "high" }.to_string(),
                    description: format!("CPU usage at {:.1}%", last.cpu_usage),
                    recommendation: "Consider optimizing CPU-intensive operations or adding caching".to_string(),
                });
            }

            // High memory usage
            let memory_percent = (last.memory_usage.used_bytes as f64 / last.memory_usage.total_bytes as f64) * 100.0;
            if memory_percent > 80.0 {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::Memory,
                    severity: if memory_percent > 95.0 { "critical" } else { "high" }.to_string(),
                    description: format!("Memory usage at {:.1}%", memory_percent),
                    recommendation: "Consider optimizing memory usage or increasing available memory".to_string(),
                });
            }

            // Slow database queries
            if last.database_metrics.avg_query_time_ms > 100.0 {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::Database,
                    severity: if last.database_metrics.avg_query_time_ms > 500.0 { "critical" } else { "high" }.to_string(),
                    description: format!("Average query time {:.1}ms", last.database_metrics.avg_query_time_ms),
                    recommendation: "Consider adding indexes or optimizing queries".to_string(),
                });
            }

            // Low cache hit rate
            if last.cache_metrics.hit_rate < 0.7 {
                bottlenecks.push(Bottleneck {
                    bottleneck_type: BottleneckType::Cache,
                    severity: "medium".to_string(),
                    description: format!("Cache hit rate only {:.1}%", last.cache_metrics.hit_rate * 100.0),
                    recommendation: "Consider increasing cache size or improving cache strategy".to_string(),
                });
            }
        }

        bottlenecks
    }
}

// Make PerformanceMonitor cloneable
impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            snapshots: self.snapshots.clone(),
            timers: self.timers.clone(),
            counters: self.counters.clone(),
            histograms: self.histograms.clone(),
            config: self.config.clone(),
            is_profiling: self.is_profiling.clone(),
        }
    }
}

// Statistics helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub p95: f64,
    pub p99: f64,
}

fn calculate_stats(values: &[f64]) -> Statistics {
    if values.is_empty() {
        return Statistics {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            p95: 0.0,
            p99: 0.0,
        };
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let sum: f64 = values.iter().sum();
    let mean = sum / values.len() as f64;

    let variance: f64 = values.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / values.len() as f64;

    Statistics {
        min: sorted[0],
        max: sorted[sorted.len() - 1],
        mean,
        median: sorted[sorted.len() / 2],
        std_dev: variance.sqrt(),
        p95: sorted[sorted.len() * 95 / 100],
        p99: sorted[sorted.len() * 99 / 100],
    }
}

fn calculate_trend(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }

    let n = values.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().sum::<f64>() / n;

    let numerator: f64 = values.iter().enumerate()
        .map(|(i, y)| (i as f64 - x_mean) * (y - y_mean))
        .sum();

    let denominator: f64 = (0..values.len())
        .map(|i| (i as f64 - x_mean).powi(2))
        .sum();

    if denominator == 0.0 {
        None
    } else {
        Some(numerator / denominator)
    }
}

// Profiling report
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfilingReport {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: u64,
    pub snapshot_count: usize,
    pub cpu_stats: Statistics,
    pub memory_stats: Statistics,
    pub response_time_stats: Statistics,
    pub slow_operations: Vec<SlowOperation>,
    pub hot_paths: Vec<HotPath>,
    pub memory_leaks: Vec<MemoryLeak>,
    pub performance_bottlenecks: Vec<Bottleneck>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlowOperation {
    pub name: String,
    pub duration_ms: f64,
    pub timestamp: DateTime<Utc>,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HotPath {
    pub name: String,
    pub call_count: usize,
    pub total_time_ms: f64,
    pub avg_time_ms: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryLeak {
    pub growth_rate_bytes_per_sec: u64,
    pub duration_seconds: u64,
    pub total_growth_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bottleneck {
    pub bottleneck_type: BottleneckType,
    pub severity: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BottleneckType {
    Cpu,
    Memory,
    Disk,
    Network,
    Database,
    Cache,
    Api,
}