// Distributed Tracing Service
// Core tracing functionality for request tracking across the application

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

use crate::error::AppError;
use super::spans::{Span, SpanKind, SpanStatus};
use super::context::TraceContext;
use super::exporters::TraceExporter;

// Trace ID type
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraceId(String);

impl Default for TraceId {
    fn default() -> Self {
        // Generate 128-bit trace ID as hex string
        Self(format!("{:032x}", Uuid::new_v4().as_u128()))
    }
}

impl TraceId {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Span ID type
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpanId(String);

impl Default for SpanId {
    fn default() -> Self {
        // Generate 64-bit span ID as hex string
        Self(format!("{:016x}", Uuid::new_v4().as_u64_pair().0))
    }
}

impl SpanId {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub sample_rate: f64, // 0.0 to 1.0
    pub max_spans_per_trace: usize,
    pub span_timeout: Duration,
    pub export_interval: Duration,
    pub max_export_batch_size: usize,
    pub propagation_formats: Vec<PropagationFormat>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_rate: 1.0,
            max_spans_per_trace: 1000,
            span_timeout: Duration::from_secs(60),
            export_interval: Duration::from_secs(5),
            max_export_batch_size: 512,
            propagation_formats: vec![PropagationFormat::W3CTraceContext],
        }
    }
}

// Propagation format for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropagationFormat {
    W3CTraceContext,
    B3,
    Jaeger,
    XRay,
}

// Active trace
struct ActiveTrace {
    trace_id: TraceId,
    root_span: Option<SpanId>,
    spans: HashMap<SpanId, Span>,
    created_at: DateTime<Utc>,
    sampled: bool,
}

// Tracing service
pub struct TracingService {
    config: TracingConfig,
    active_traces: Arc<RwLock<HashMap<TraceId, ActiveTrace>>>,
    exporters: Arc<RwLock<Vec<Box<dyn TraceExporter>>>>,
    pending_exports: Arc<RwLock<Vec<Span>>>,
}

impl TracingService {
    // Create new tracing service
    pub fn new(config: TracingConfig) -> Self {
        Self {
            config,
            active_traces: Arc::new(RwLock::new(HashMap::new())),
            exporters: Arc::new(RwLock::new(Vec::new())),
            pending_exports: Arc::new(RwLock::new(Vec::new())),
        }
    }

    // Start a new trace
    pub async fn start_trace(
        &self,
        operation_name: &str,
        kind: SpanKind,
    ) -> Result<(TraceId, SpanId), AppError> {
        // Check if tracing is enabled
        if !self.config.enabled {
            return Err(AppError::ValidationError {
                message: "Tracing is disabled".to_string(),
            });
        }

        // Sampling decision
        let sampled = self.should_sample();

        // Create trace and root span
        let trace_id = TraceId::new();
        let span_id = SpanId::new();

        // Create root span
        let root_span = Span::new(
            trace_id.clone(),
            span_id.clone(),
            None,
            operation_name.to_string(),
            kind,
        );

        // Store active trace
        let mut traces = self.active_traces.write().await;
        traces.insert(
            trace_id.clone(),
            ActiveTrace {
                trace_id: trace_id.clone(),
                root_span: Some(span_id.clone()),
                spans: HashMap::from([(span_id.clone(), root_span)]),
                created_at: Utc::now(),
                sampled,
            },
        );

        info!(
            trace_id = %trace_id.as_str(),
            span_id = %span_id.as_str(),
            operation = operation_name,
            sampled = sampled,
            "Started new trace"
        );

        Ok((trace_id, span_id))
    }

    // Start a child span
    pub async fn start_span(
        &self,
        trace_id: &TraceId,
        parent_span_id: Option<&SpanId>,
        operation_name: &str,
        kind: SpanKind,
    ) -> Result<SpanId, AppError> {
        // Check trace exists
        let mut traces = self.active_traces.write().await;
        let trace = traces.get_mut(trace_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Trace {} not found", trace_id.as_str()),
            }
        })?;

        // Check span limit
        if trace.spans.len() >= self.config.max_spans_per_trace {
            return Err(AppError::ValidationError {
                message: "Maximum spans per trace exceeded".to_string(),
            });
        }

        // Create new span
        let span_id = SpanId::new();
        let span = Span::new(
            trace_id.clone(),
            span_id.clone(),
            parent_span_id.cloned(),
            operation_name.to_string(),
            kind,
        );

        // Store span
        trace.spans.insert(span_id.clone(), span);

        debug!(
            trace_id = %trace_id.as_str(),
            span_id = %span_id.as_str(),
            parent = ?parent_span_id.map(|s| s.as_str()),
            operation = operation_name,
            "Started span"
        );

        Ok(span_id)
    }

    // End a span
    pub async fn end_span(
        &self,
        trace_id: &TraceId,
        span_id: &SpanId,
        status: SpanStatus,
    ) -> Result<(), AppError> {
        // Get trace and span
        let mut traces = self.active_traces.write().await;
        let trace = traces.get_mut(trace_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Trace {} not found", trace_id.as_str()),
            }
        })?;

        let span = trace.spans.get_mut(span_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Span {} not found", span_id.as_str()),
            }
        })?;

        // End span
        span.end(status.clone());

        debug!(
            trace_id = %trace_id.as_str(),
            span_id = %span_id.as_str(),
            status = ?status,
            duration_ms = span.duration_ms(),
            "Ended span"
        );

        // If sampled, add to export queue
        if trace.sampled {
            let mut pending = self.pending_exports.write().await;
            pending.push(span.clone());
        }

        // Check if trace is complete (all spans ended)
        let all_ended = trace.spans.values().all(|s| s.is_ended());
        if all_ended {
            // Export trace
            if trace.sampled {
                self.export_trace(&trace).await?;
            }

            // Remove from active traces
            let trace_id_clone = trace_id.clone();
            let _ = trace; // Release borrow
            traces.remove(&trace_id_clone);

            info!(
                trace_id = %trace_id.as_str(),
                "Completed trace"
            );
        }

        Ok(())
    }

    // Add attribute to span
    pub async fn add_span_attribute(
        &self,
        trace_id: &TraceId,
        span_id: &SpanId,
        key: String,
        value: serde_json::Value,
    ) -> Result<(), AppError> {
        let mut traces = self.active_traces.write().await;
        let trace = traces.get_mut(trace_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Trace {} not found", trace_id.as_str()),
            }
        })?;

        let span = trace.spans.get_mut(span_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Span {} not found", span_id.as_str()),
            }
        })?;

        span.add_attribute(key, value);
        Ok(())
    }

    // Add event to span
    pub async fn add_span_event(
        &self,
        trace_id: &TraceId,
        span_id: &SpanId,
        name: String,
        attributes: HashMap<String, serde_json::Value>,
    ) -> Result<(), AppError> {
        let mut traces = self.active_traces.write().await;
        let trace = traces.get_mut(trace_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Trace {} not found", trace_id.as_str()),
            }
        })?;

        let span = trace.spans.get_mut(span_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Span {} not found", span_id.as_str()),
            }
        })?;

        span.add_event(name, attributes);
        Ok(())
    }

    // Record exception in span
    pub async fn record_exception(
        &self,
        trace_id: &TraceId,
        span_id: &SpanId,
        error: &AppError,
    ) -> Result<(), AppError> {
        let mut attributes = HashMap::new();
        attributes.insert("exception.type".to_string(), serde_json::json!(error.error_type_name()));
        attributes.insert("exception.message".to_string(), serde_json::json!(error.to_string()));

        self.add_span_event(
            trace_id,
            span_id,
            "exception".to_string(),
            attributes,
        ).await
    }

    // Get current trace context
    pub async fn get_context(&self, trace_id: &TraceId) -> Result<TraceContext, AppError> {
        let traces = self.active_traces.read().await;
        let trace = traces.get(trace_id).ok_or_else(|| {
            AppError::NotFound {
                message: format!("Trace {} not found", trace_id.as_str()),
            }
        })?;

        Ok(TraceContext::new(
            trace_id.clone(),
            trace.root_span.clone(),
            trace.sampled,
        ))
    }

    // Inject trace context into headers
    pub fn inject_context(
        &self,
        context: &TraceContext,
        headers: &mut HashMap<String, String>,
    ) {
        for format in &self.config.propagation_formats {
            match format {
                PropagationFormat::W3CTraceContext => {
                    // W3C Trace Context format
                    let traceparent = format!(
                        "00-{}-{}-{}",
                        context.trace_id.as_str(),
                        context.span_id.as_ref().map_or("0000000000000000", |s| s.as_str()),
                        if context.sampled { "01" } else { "00" }
                    );
                    headers.insert("traceparent".to_string(), traceparent);

                    // Add tracestate if present
                    if let Some(state) = context.trace_state.as_ref() {
                        headers.insert("tracestate".to_string(), state.clone());
                    }
                }
                PropagationFormat::B3 => {
                    // B3 propagation format
                    headers.insert("X-B3-TraceId".to_string(), context.trace_id.as_str().to_string());
                    if let Some(span_id) = &context.span_id {
                        headers.insert("X-B3-SpanId".to_string(), span_id.as_str().to_string());
                    }
                    headers.insert("X-B3-Sampled".to_string(), if context.sampled { "1".to_string() } else { "0".to_string() });
                }
                PropagationFormat::Jaeger => {
                    // Jaeger propagation format
                    let uber_trace_id = format!(
                        "{}:{}:0:{}",
                        context.trace_id.as_str(),
                        context.span_id.as_ref().map_or("0", |s| s.as_str()),
                        if context.sampled { "1" } else { "0" }
                    );
                    headers.insert("uber-trace-id".to_string(), uber_trace_id);
                }
                PropagationFormat::XRay => {
                    // AWS X-Ray format
                    let trace_header = format!(
                        "Root={};Sampled={}",
                        context.trace_id.as_str(),
                        if context.sampled { "1" } else { "0" }
                    );
                    headers.insert("X-Amzn-Trace-Id".to_string(), trace_header);
                }
            }
        }
    }

    // Extract trace context from headers
    pub fn extract_context(
        &self,
        headers: &HashMap<String, String>,
    ) -> Option<TraceContext> {
        // Try each propagation format
        for format in &self.config.propagation_formats {
            let context = match format {
                PropagationFormat::W3CTraceContext => {
                    // Extract W3C Trace Context
                    if let Some(traceparent) = headers.get("traceparent") {
                        let parts: Vec<&str> = traceparent.split('-').collect();
                        if parts.len() >= 4 {
                            let trace_id = TraceId::from_string(parts[1].to_string());
                            let span_id = if parts[2] != "0000000000000000" {
                                Some(SpanId::from_string(parts[2].to_string()))
                            } else {
                                None
                            };
                            let sampled = parts[3] == "01";

                            let mut context = TraceContext::new(trace_id, span_id, sampled);

                            // Extract tracestate
                            if let Some(state) = headers.get("tracestate") {
                                context.trace_state = Some(state.clone());
                            }

                            return Some(context);
                        }
                    }
                    None
                }
                PropagationFormat::B3 => {
                    // Extract B3 format
                    if let Some(trace_id) = headers.get("X-B3-TraceId") {
                        let span_id = headers.get("X-B3-SpanId")
                            .map(|s| SpanId::from_string(s.clone()));
                        let sampled = headers.get("X-B3-Sampled")
                            .map_or(false, |s| s == "1");

                        return Some(TraceContext::new(
                            TraceId::from_string(trace_id.clone()),
                            span_id,
                            sampled,
                        ));
                    }
                    None
                }
                PropagationFormat::Jaeger => {
                    // Extract Jaeger format
                    if let Some(uber_trace_id) = headers.get("uber-trace-id") {
                        let parts: Vec<&str> = uber_trace_id.split(':').collect();
                        if parts.len() >= 4 {
                            let trace_id = TraceId::from_string(parts[0].to_string());
                            let span_id = if parts[1] != "0" {
                                Some(SpanId::from_string(parts[1].to_string()))
                            } else {
                                None
                            };
                            let sampled = parts[3] == "1";

                            return Some(TraceContext::new(trace_id, span_id, sampled));
                        }
                    }
                    None
                }
                PropagationFormat::XRay => {
                    // Extract X-Ray format
                    if let Some(trace_header) = headers.get("X-Amzn-Trace-Id") {
                        let mut trace_id = None;
                        let mut sampled = false;

                        for part in trace_header.split(';') {
                            let kv: Vec<&str> = part.split('=').collect();
                            if kv.len() == 2 {
                                match kv[0] {
                                    "Root" => trace_id = Some(TraceId::from_string(kv[1].to_string())),
                                    "Sampled" => sampled = kv[1] == "1",
                                    _ => {}
                                }
                            }
                        }

                        if let Some(tid) = trace_id {
                            return Some(TraceContext::new(tid, None, sampled));
                        }
                    }
                    None
                }
            };

            if context.is_some() {
                return context;
            }
        }

        None
    }

    // Add exporter
    pub async fn add_exporter(&self, exporter: Box<dyn TraceExporter>) {
        let mut exporters = self.exporters.write().await;
        exporters.push(exporter);
    }

    // Export trace
    async fn export_trace(&self, trace: &ActiveTrace) -> Result<(), AppError> {
        let exporters = self.exporters.read().await;

        // Collect all spans
        let spans: Vec<Span> = trace.spans.values().cloned().collect();

        // Export to all configured exporters
        for exporter in exporters.iter() {
            if let Err(e) = exporter.export(&spans).await {
                warn!(
                    trace_id = %trace.trace_id.as_str(),
                    error = %e,
                    "Failed to export trace"
                );
            }
        }

        Ok(())
    }

    // Export pending spans (batch export)
    pub async fn export_pending(&self) -> Result<(), AppError> {
        let mut pending = self.pending_exports.write().await;
        if pending.is_empty() {
            return Ok(());
        }

        // Take up to max batch size
        let batch_size = self.config.max_export_batch_size.min(pending.len());
        let batch: Vec<Span> = pending.drain(..batch_size).collect();

        // Export batch
        let exporters = self.exporters.read().await;
        for exporter in exporters.iter() {
            if let Err(e) = exporter.export(&batch).await {
                warn!(
                    error = %e,
                    batch_size = batch.len(),
                    "Failed to export span batch"
                );
            }
        }

        Ok(())
    }

    // Sampling decision
    fn should_sample(&self) -> bool {
        if self.config.sample_rate >= 1.0 {
            return true;
        }
        if self.config.sample_rate <= 0.0 {
            return false;
        }

        // Random sampling
        rand::random::<f64>() < self.config.sample_rate
    }

    // Clean up old traces
    pub async fn cleanup_old_traces(&self) -> Result<(), AppError> {
        let timeout = self.config.span_timeout;
        let now = Utc::now();

        let mut traces = self.active_traces.write().await;
        let old_traces: Vec<TraceId> = traces
            .iter()
            .filter(|(_, trace)| {
                now.signed_duration_since(trace.created_at).num_seconds() as u64 > timeout.as_secs()
            })
            .map(|(id, _)| id.clone())
            .collect();

        for trace_id in old_traces {
            traces.remove(&trace_id);
            warn!(
                trace_id = %trace_id.as_str(),
                "Removed old trace due to timeout"
            );
        }

        Ok(())
    }

    // Get statistics
    pub async fn get_stats(&self) -> TracingStats {
        let traces = self.active_traces.read().await;
        let pending = self.pending_exports.read().await;

        let total_spans: usize = traces.values().map(|t| t.spans.len()).sum();
        let sampled_traces = traces.values().filter(|t| t.sampled).count();

        TracingStats {
            active_traces: traces.len(),
            total_spans,
            sampled_traces,
            pending_exports: pending.len(),
            sample_rate: self.config.sample_rate,
        }
    }
}

// Tracing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingStats {
    pub active_traces: usize,
    pub total_spans: usize,
    pub sampled_traces: usize,
    pub pending_exports: usize,
    pub sample_rate: f64,
}