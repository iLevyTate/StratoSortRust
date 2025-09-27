// Trace Exporters
// Export trace data to various backends

use async_trait::async_trait;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use serde_json;
use std::io::Write;
use std::path::PathBuf;

use crate::error::AppError;
use super::spans::Span;

// Trace exporter trait
#[async_trait]
pub trait TraceExporter: Send + Sync {
    // Export spans
    async fn export(&self, spans: &[Span]) -> Result<(), AppError>;

    // Flush any pending data
    async fn flush(&self) -> Result<(), AppError>;

    // Shutdown exporter
    async fn shutdown(&self) -> Result<(), AppError>;
}

// Exporter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub timeout: Duration,
    pub batch_size: usize,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: None,
            timeout: Duration::from_secs(10),
            batch_size: 512,
            retry_attempts: 3,
            retry_delay: Duration::from_secs(1),
        }
    }
}

// Console exporter (for debugging)
pub struct ConsoleExporter {
    config: ExporterConfig,
}

impl ConsoleExporter {
    pub fn new(config: ExporterConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl TraceExporter for ConsoleExporter {
    async fn export(&self, spans: &[Span]) -> Result<(), AppError> {
        if !self.config.enabled {
            return Ok(());
        }

        for span in spans {
            println!("=== SPAN ===");
            println!("Trace ID: {}", span.trace_id.as_str());
            println!("Span ID: {}", span.span_id.as_str());
            println!("Parent ID: {:?}", span.parent_span_id.as_ref().map(|s| s.as_str()));
            println!("Operation: {}", span.operation_name);
            println!("Kind: {:?}", span.kind);
            println!("Status: {:?}", span.status);
            println!("Duration: {:?}ms", span.duration_ms());
            println!("Attributes: {:?}", span.attributes);
            println!("Events: {} events", span.events.len());
            println!("Links: {} links", span.links.len());
            println!("===========\n");
        }

        Ok(())
    }

    async fn flush(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        Ok(())
    }
}

// JSON file exporter
pub struct JsonFileExporter {
    config: ExporterConfig,
    file_path: PathBuf,
}

impl JsonFileExporter {
    pub fn new(config: ExporterConfig, file_path: PathBuf) -> Self {
        Self { config, file_path }
    }
}

#[async_trait]
impl TraceExporter for JsonFileExporter {
    async fn export(&self, spans: &[Span]) -> Result<(), AppError> {
        if !self.config.enabled {
            return Ok(());
        }

        // Serialize spans to JSON
        let json_data = serde_json::to_string_pretty(spans)
            .map_err(|e| AppError::SerializationError {
                message: format!("Failed to serialize spans: {}", e),
            })?;

        // Write to file (append mode)
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to open trace file: {}", e),
            })?;

        writeln!(file, "{}", json_data)
            .map_err(|e| AppError::IoError {
                message: format!("Failed to write traces: {}", e),
            })?;

        Ok(())
    }

    async fn flush(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        Ok(())
    }
}

// OTLP exporter (OpenTelemetry Protocol)
pub struct OtlpExporter {
    config: ExporterConfig,
    client: reqwest::Client,
}

impl OtlpExporter {
    pub fn new(config: ExporterConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    // Convert spans to OTLP format
    fn to_otlp_format(&self, spans: &[Span]) -> serde_json::Value {
        let resource_spans = serde_json::json!({
            "resource": {
                "attributes": [
                    {
                        "key": "service.name",
                        "value": {
                            "stringValue": "stratosort"
                        }
                    },
                    {
                        "key": "service.version",
                        "value": {
                            "stringValue": env!("CARGO_PKG_VERSION")
                        }
                    }
                ]
            },
            "scopeSpans": [
                {
                    "scope": {
                        "name": "stratosort-tracer",
                        "version": "1.0.0"
                    },
                    "spans": spans.iter().map(|span| {
                        serde_json::json!({
                            "traceId": span.trace_id.as_str(),
                            "spanId": span.span_id.as_str(),
                            "parentSpanId": span.parent_span_id.as_ref().map(|s| s.as_str()),
                            "name": span.operation_name,
                            "kind": self.span_kind_to_otlp(&span.kind),
                            "startTimeUnixNano": span.start_time.timestamp_nanos_opt().unwrap_or(0),
                            "endTimeUnixNano": span.end_time.map(|t| t.timestamp_nanos_opt().unwrap_or(0)),
                            "attributes": self.attributes_to_otlp(&span.attributes),
                            "events": span.events.iter().map(|event| {
                                serde_json::json!({
                                    "timeUnixNano": event.timestamp.timestamp_nanos_opt().unwrap_or(0),
                                    "name": event.name,
                                    "attributes": self.attributes_to_otlp(&event.attributes)
                                })
                            }).collect::<Vec<_>>(),
                            "links": span.links.iter().map(|link| {
                                serde_json::json!({
                                    "traceId": link.trace_id.as_str(),
                                    "spanId": link.span_id.as_str(),
                                    "attributes": self.attributes_to_otlp(&link.attributes)
                                })
                            }).collect::<Vec<_>>(),
                            "status": self.status_to_otlp(&span.status)
                        })
                    }).collect::<Vec<_>>()
                }
            ]
        });

        serde_json::json!({
            "resourceSpans": [resource_spans]
        })
    }

    // Convert span kind to OTLP format
    fn span_kind_to_otlp(&self, kind: &super::spans::SpanKind) -> u32 {
        match kind {
            super::spans::SpanKind::Internal => 1,
            super::spans::SpanKind::Server => 2,
            super::spans::SpanKind::Client => 3,
            super::spans::SpanKind::Producer => 4,
            super::spans::SpanKind::Consumer => 5,
        }
    }

    // Convert status to OTLP format
    fn status_to_otlp(&self, status: &super::spans::SpanStatus) -> serde_json::Value {
        match status {
            super::spans::SpanStatus::Unset => serde_json::json!({
                "code": 0
            }),
            super::spans::SpanStatus::Ok => serde_json::json!({
                "code": 1
            }),
            super::spans::SpanStatus::Error(msg) => serde_json::json!({
                "code": 2,
                "message": msg
            }),
        }
    }

    // Convert attributes to OTLP format
    fn attributes_to_otlp(&self, attributes: &std::collections::HashMap<String, serde_json::Value>) -> Vec<serde_json::Value> {
        attributes.iter().map(|(key, value)| {
            let attr_value = match value {
                serde_json::Value::String(s) => serde_json::json!({ "stringValue": s }),
                serde_json::Value::Number(n) => {
                    if n.is_i64() {
                        serde_json::json!({ "intValue": n.as_i64() })
                    } else {
                        serde_json::json!({ "doubleValue": n.as_f64() })
                    }
                },
                serde_json::Value::Bool(b) => serde_json::json!({ "boolValue": b }),
                _ => serde_json::json!({ "stringValue": value.to_string() }),
            };

            serde_json::json!({
                "key": key,
                "value": attr_value
            })
        }).collect()
    }
}

#[async_trait]
impl TraceExporter for OtlpExporter {
    async fn export(&self, spans: &[Span]) -> Result<(), AppError> {
        if !self.config.enabled {
            return Ok(());
        }

        let endpoint = self.config.endpoint.as_ref()
            .ok_or_else(|| AppError::ConfigError {
                message: "OTLP endpoint not configured".to_string(),
            })?;

        // Convert to OTLP format
        let otlp_data = self.to_otlp_format(spans);

        // Send to endpoint with retries
        let mut attempts = 0;
        loop {
            attempts += 1;

            match self.client
                .post(endpoint)
                .header("Content-Type", "application/json")
                .json(&otlp_data)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                }
                Ok(response) => {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();

                    if attempts >= self.config.retry_attempts {
                        return Err(AppError::ExternalServiceError {
                            service: "OTLP".to_string(),
                            message: format!("Failed to export traces: {} - {}", status, error_text),
                        });
                    }

                    // Retry after delay
                    tokio::time::sleep(self.config.retry_delay).await;
                }
                Err(e) => {
                    if attempts >= self.config.retry_attempts {
                        return Err(AppError::ExternalServiceError {
                            service: "OTLP".to_string(),
                            message: format!("Failed to export traces: {}", e),
                        });
                    }

                    // Retry after delay
                    tokio::time::sleep(self.config.retry_delay).await;
                }
            }
        }
    }

    async fn flush(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        Ok(())
    }
}

// Jaeger exporter
pub struct JaegerExporter {
    config: ExporterConfig,
    client: reqwest::Client,
}

impl JaegerExporter {
    pub fn new(config: ExporterConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    // Convert spans to Jaeger Thrift format (JSON representation)
    fn to_jaeger_format(&self, spans: &[Span]) -> serde_json::Value {
        // Group spans by trace
        let mut traces: std::collections::HashMap<String, Vec<&Span>> = std::collections::HashMap::new();
        for span in spans {
            traces.entry(span.trace_id.as_str().to_string())
                .or_insert_with(Vec::new)
                .push(span);
        }

        // Convert to Jaeger batch format
        let batches: Vec<serde_json::Value> = traces.into_iter().map(|(_trace_id, trace_spans)| {
            serde_json::json!({
                "process": {
                    "serviceName": "stratosort",
                    "tags": [
                        {
                            "key": "service.version",
                            "type": "string",
                            "value": env!("CARGO_PKG_VERSION")
                        }
                    ]
                },
                "spans": trace_spans.iter().map(|span| {
                    serde_json::json!({
                        "traceID": span.trace_id.as_str(),
                        "spanID": span.span_id.as_str(),
                        "parentSpanID": span.parent_span_id.as_ref().map(|s| s.as_str()),
                        "operationName": span.operation_name,
                        "startTime": span.start_time.timestamp_micros(),
                        "duration": span.duration_ns.unwrap_or(0) / 1000, // Convert to microseconds
                        "tags": self.attributes_to_jaeger_tags(&span.attributes),
                        "logs": span.events.iter().map(|event| {
                            serde_json::json!({
                                "timestamp": event.timestamp.timestamp_micros(),
                                "fields": self.attributes_to_jaeger_tags(&event.attributes)
                            })
                        }).collect::<Vec<_>>(),
                        "references": span.links.iter().map(|link| {
                            serde_json::json!({
                                "refType": "FOLLOWS_FROM",
                                "traceID": link.trace_id.as_str(),
                                "spanID": link.span_id.as_str()
                            })
                        }).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>()
            })
        }).collect();

        serde_json::json!({ "batches": batches })
    }

    // Convert attributes to Jaeger tags
    fn attributes_to_jaeger_tags(&self, attributes: &std::collections::HashMap<String, serde_json::Value>) -> Vec<serde_json::Value> {
        attributes.iter().map(|(key, value)| {
            match value {
                serde_json::Value::String(s) => serde_json::json!({
                    "key": key,
                    "type": "string",
                    "value": s
                }),
                serde_json::Value::Number(n) => {
                    if n.is_i64() {
                        serde_json::json!({
                            "key": key,
                            "type": "long",
                            "value": n.as_i64()
                        })
                    } else {
                        serde_json::json!({
                            "key": key,
                            "type": "double",
                            "value": n.as_f64()
                        })
                    }
                },
                serde_json::Value::Bool(b) => serde_json::json!({
                    "key": key,
                    "type": "bool",
                    "value": b
                }),
                _ => serde_json::json!({
                    "key": key,
                    "type": "string",
                    "value": value.to_string()
                }),
            }
        }).collect()
    }
}

#[async_trait]
impl TraceExporter for JaegerExporter {
    async fn export(&self, spans: &[Span]) -> Result<(), AppError> {
        if !self.config.enabled {
            return Ok(());
        }

        let endpoint = self.config.endpoint.as_ref()
            .ok_or_else(|| AppError::ConfigError {
                message: "Jaeger endpoint not configured".to_string(),
            })?;

        // Convert to Jaeger format
        let jaeger_data = self.to_jaeger_format(spans);

        // Send to Jaeger collector
        let response = self.client
            .post(format!("{}/api/traces", endpoint))
            .header("Content-Type", "application/json")
            .json(&jaeger_data)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                service: "Jaeger".to_string(),
                message: format!("Failed to export traces: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError {
                service: "Jaeger".to_string(),
                message: format!("Failed to export traces: {}", response.status()),
            });
        }

        Ok(())
    }

    async fn flush(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        Ok(())
    }
}