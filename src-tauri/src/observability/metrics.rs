// Metrics Collection
// Metrics tracking and aggregation for observability

use std::sync::Arc;
use std::time::Instant;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};


// Metric types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

// Metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary { count: u64, sum: f64, quantiles: Vec<(f64, f64)> },
}

// Metric point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub name: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

// Metric descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDescriptor {
    pub name: String,
    pub metric_type: MetricType,
    pub description: String,
    pub unit: String,
    pub labels: Vec<String>,
}

// Counter metric
pub struct Counter {
    name: String,
    value: Arc<RwLock<u64>>,
    labels: HashMap<String, String>,
}

impl Counter {
    pub fn new(name: String, labels: HashMap<String, String>) -> Self {
        Self {
            name,
            value: Arc::new(RwLock::new(0)),
            labels,
        }
    }

    pub async fn increment(&self) {
        let mut value = self.value.write().await;
        *value += 1;
    }

    pub async fn add(&self, delta: u64) {
        let mut value = self.value.write().await;
        *value += delta;
    }

    pub async fn get(&self) -> u64 {
        *self.value.read().await
    }

    pub async fn reset(&self) {
        let mut value = self.value.write().await;
        *value = 0;
    }

    pub fn to_metric_point(&self, value: u64) -> MetricPoint {
        MetricPoint {
            name: self.name.clone(),
            metric_type: MetricType::Counter,
            value: MetricValue::Counter(value),
            labels: self.labels.clone(),
            timestamp: Utc::now(),
        }
    }
}

// Gauge metric
pub struct Gauge {
    name: String,
    value: Arc<RwLock<f64>>,
    labels: HashMap<String, String>,
}

impl Gauge {
    pub fn new(name: String, labels: HashMap<String, String>) -> Self {
        Self {
            name,
            value: Arc::new(RwLock::new(0.0)),
            labels,
        }
    }

    pub async fn set(&self, value: f64) {
        let mut current = self.value.write().await;
        *current = value;
    }

    pub async fn increment(&self, delta: f64) {
        let mut value = self.value.write().await;
        *value += delta;
    }

    pub async fn decrement(&self, delta: f64) {
        let mut value = self.value.write().await;
        *value -= delta;
    }

    pub async fn get(&self) -> f64 {
        *self.value.read().await
    }

    pub fn to_metric_point(&self, value: f64) -> MetricPoint {
        MetricPoint {
            name: self.name.clone(),
            metric_type: MetricType::Gauge,
            value: MetricValue::Gauge(value),
            labels: self.labels.clone(),
            timestamp: Utc::now(),
        }
    }
}

// Histogram metric
pub struct Histogram {
    name: String,
    buckets: Vec<f64>,
    counts: Arc<RwLock<Vec<u64>>>,
    sum: Arc<RwLock<f64>>,
    count: Arc<RwLock<u64>>,
    labels: HashMap<String, String>,
}

impl Histogram {
    pub fn new(name: String, buckets: Vec<f64>, labels: HashMap<String, String>) -> Self {
        let counts = vec![0; buckets.len() + 1]; // +1 for infinity bucket
        Self {
            name,
            buckets,
            counts: Arc::new(RwLock::new(counts)),
            sum: Arc::new(RwLock::new(0.0)),
            count: Arc::new(RwLock::new(0)),
            labels,
        }
    }

    pub async fn observe(&self, value: f64) {
        let mut counts = self.counts.write().await;
        let mut sum = self.sum.write().await;
        let mut count = self.count.write().await;

        // Find appropriate bucket
        let mut bucket_index = self.buckets.len();
        for (i, &bucket) in self.buckets.iter().enumerate() {
            if value <= bucket {
                bucket_index = i;
                break;
            }
        }

        // Update counts
        counts[bucket_index] += 1;
        *sum += value;
        *count += 1;
    }

    pub async fn get_stats(&self) -> (u64, f64, Vec<u64>) {
        let count = *self.count.read().await;
        let sum = *self.sum.read().await;
        let counts = self.counts.read().await.clone();
        (count, sum, counts)
    }

    pub fn to_metric_point(&self, values: Vec<f64>) -> MetricPoint {
        MetricPoint {
            name: self.name.clone(),
            metric_type: MetricType::Histogram,
            value: MetricValue::Histogram(values),
            labels: self.labels.clone(),
            timestamp: Utc::now(),
        }
    }
}

// Timer for measuring durations
pub struct Timer {
    start: Instant,
    histogram: Arc<Histogram>,
}

impl Timer {
    pub fn new(histogram: Arc<Histogram>) -> Self {
        Self {
            start: Instant::now(),
            histogram,
        }
    }

    pub async fn stop(self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.observe(duration).await;
    }
}

// Metrics collector
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
    descriptors: Arc<RwLock<HashMap<String, MetricDescriptor>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            descriptors: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    // Register metric descriptor
    pub async fn register_metric(&self, descriptor: MetricDescriptor) {
        let mut descriptors = self.descriptors.write().await;
        descriptors.insert(descriptor.name.clone(), descriptor);
    }

    // Create or get counter
    pub async fn counter(&self, name: &str, labels: HashMap<String, String>) -> Arc<Counter> {
        let key = self.metric_key(name, &labels);

        let mut counters = self.counters.write().await;
        if let Some(counter) = counters.get(&key) {
            return counter.clone();
        }

        let counter = Arc::new(Counter::new(name.to_string(), labels));
        counters.insert(key, counter.clone());
        counter
    }

    // Create or get gauge
    pub async fn gauge(&self, name: &str, labels: HashMap<String, String>) -> Arc<Gauge> {
        let key = self.metric_key(name, &labels);

        let mut gauges = self.gauges.write().await;
        if let Some(gauge) = gauges.get(&key) {
            return gauge.clone();
        }

        let gauge = Arc::new(Gauge::new(name.to_string(), labels));
        gauges.insert(key, gauge.clone());
        gauge
    }

    // Create or get histogram
    pub async fn histogram(
        &self,
        name: &str,
        buckets: Vec<f64>,
        labels: HashMap<String, String>,
    ) -> Arc<Histogram> {
        let key = self.metric_key(name, &labels);

        let mut histograms = self.histograms.write().await;
        if let Some(histogram) = histograms.get(&key) {
            return histogram.clone();
        }

        let histogram = Arc::new(Histogram::new(name.to_string(), buckets, labels));
        histograms.insert(key, histogram.clone());
        histogram
    }

    // Start timer
    pub async fn start_timer(&self, name: &str, labels: HashMap<String, String>) -> Timer {
        let histogram = self.histogram(
            name,
            vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
            labels,
        ).await;

        Timer::new(histogram)
    }

    // Collect all metrics
    pub async fn collect(&self) -> Vec<MetricPoint> {
        let mut points = Vec::new();

        // Collect counters
        let counters = self.counters.read().await;
        for counter in counters.values() {
            let value = counter.get().await;
            points.push(counter.to_metric_point(value));
        }

        // Collect gauges
        let gauges = self.gauges.read().await;
        for gauge in gauges.values() {
            let value = gauge.get().await;
            points.push(gauge.to_metric_point(value));
        }

        // Collect histograms
        let histograms = self.histograms.read().await;
        for histogram in histograms.values() {
            let (count, sum, _counts) = histogram.get_stats().await;

            // Convert to values
            let values = vec![count as f64, sum];
            points.push(histogram.to_metric_point(values));
        }

        points
    }

    // Generate metric key with labels
    fn metric_key(&self, name: &str, labels: &HashMap<String, String>) -> String {
        let mut key = name.to_string();

        // Sort labels for consistent key
        let mut label_pairs: Vec<(&String, &String)> = labels.iter().collect();
        label_pairs.sort_by_key(|&(k, _)| k);

        for (label_name, label_value) in label_pairs {
            key.push_str(&format!(",{}={}", label_name, label_value));
        }

        key
    }

    // Export metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();
        let descriptors = self.descriptors.read().await;

        // Export counters
        let counters = self.counters.read().await;
        for counter in counters.values() {
            let value = counter.get().await;
            let name = &counter.name;

            // Add descriptor comment if available
            if let Some(desc) = descriptors.get(name) {
                output.push_str(&format!("# HELP {} {}\n", name, desc.description));
                output.push_str(&format!("# TYPE {} counter\n", name));
            }

            output.push_str(&name.to_string());
            if !counter.labels.is_empty() {
                output.push('{');
                let labels: Vec<String> = counter.labels.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                output.push_str(&labels.join(","));
                output.push('}');
            }
            output.push_str(&format!(" {}\n", value));
        }

        // Export gauges
        let gauges = self.gauges.read().await;
        for gauge in gauges.values() {
            let value = gauge.get().await;
            let name = &gauge.name;

            // Add descriptor comment if available
            if let Some(desc) = descriptors.get(name) {
                output.push_str(&format!("# HELP {} {}\n", name, desc.description));
                output.push_str(&format!("# TYPE {} gauge\n", name));
            }

            output.push_str(&name.to_string());
            if !gauge.labels.is_empty() {
                output.push('{');
                let labels: Vec<String> = gauge.labels.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                output.push_str(&labels.join(","));
                output.push('}');
            }
            output.push_str(&format!(" {}\n", value));
        }

        output
    }
}