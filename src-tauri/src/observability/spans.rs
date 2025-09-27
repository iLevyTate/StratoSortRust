// Span Management
// Span data structures and operations for distributed tracing

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use super::tracing::{TraceId, SpanId};

// Span kind
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanKind {
    Server,     // Server-side handling of RPC or HTTP request
    Client,     // Client-side RPC or HTTP request
    Producer,   // Async message production
    Consumer,   // Async message consumption
    Internal,   // Internal operation
}

// Span status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Unset,
    Ok,
    Error(String),
}

// Span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub timestamp: DateTime<Utc>,
    pub name: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

// Span link (for batch operations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLink {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub attributes: HashMap<String, serde_json::Value>,
}

// Span data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    // Identifiers
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,

    // Basic info
    pub operation_name: String,
    pub kind: SpanKind,
    pub status: SpanStatus,

    // Timing
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ns: Option<u64>,

    // Attributes
    pub attributes: HashMap<String, serde_json::Value>,

    // Events
    pub events: Vec<SpanEvent>,

    // Links to other spans
    pub links: Vec<SpanLink>,

    // Resource information
    pub service_name: Option<String>,
    pub service_version: Option<String>,
    pub service_instance: Option<String>,
}

impl Span {
    // Create new span
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        parent_span_id: Option<SpanId>,
        operation_name: String,
        kind: SpanKind,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id,
            operation_name,
            kind,
            status: SpanStatus::Unset,
            start_time: Utc::now(),
            end_time: None,
            duration_ns: None,
            attributes: HashMap::new(),
            events: Vec::new(),
            links: Vec::new(),
            service_name: None,
            service_version: None,
            service_instance: None,
        }
    }

    // End span
    pub fn end(&mut self, status: SpanStatus) {
        if self.end_time.is_some() {
            return; // Already ended
        }

        self.status = status;
        self.end_time = Some(Utc::now());

        // Calculate duration
        if let Some(end) = self.end_time {
            let duration = end.signed_duration_since(self.start_time);
            self.duration_ns = Some(duration.num_nanoseconds().unwrap_or(0) as u64);
        }
    }

    // Check if span is ended
    pub fn is_ended(&self) -> bool {
        self.end_time.is_some()
    }

    // Get duration in milliseconds
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ns.map(|ns| ns / 1_000_000)
    }

    // Add attribute
    pub fn add_attribute(&mut self, key: String, value: serde_json::Value) {
        self.attributes.insert(key, value);
    }

    // Add multiple attributes
    pub fn add_attributes(&mut self, attributes: HashMap<String, serde_json::Value>) {
        self.attributes.extend(attributes);
    }

    // Set standard attributes
    pub fn set_standard_attributes(&mut self) {
        // HTTP attributes (if applicable)
        self.add_attribute("span.kind".to_string(), serde_json::json!(self.kind));

        // Service attributes
        if let Some(ref name) = self.service_name {
            self.add_attribute("service.name".to_string(), serde_json::json!(name));
        }
        if let Some(ref version) = self.service_version {
            self.add_attribute("service.version".to_string(), serde_json::json!(version));
        }
        if let Some(ref instance) = self.service_instance {
            self.add_attribute("service.instance.id".to_string(), serde_json::json!(instance));
        }
    }

    // Add event
    pub fn add_event(&mut self, name: String, attributes: HashMap<String, serde_json::Value>) {
        self.events.push(SpanEvent {
            timestamp: Utc::now(),
            name,
            attributes,
        });
    }

    // Add link to another span
    pub fn add_link(
        &mut self,
        trace_id: TraceId,
        span_id: SpanId,
        attributes: HashMap<String, serde_json::Value>,
    ) {
        self.links.push(SpanLink {
            trace_id,
            span_id,
            attributes,
        });
    }

    // Set HTTP request attributes
    pub fn set_http_request_attributes(
        &mut self,
        method: &str,
        url: &str,
        target: &str,
        host: &str,
        scheme: &str,
        status_code: Option<u16>,
        user_agent: Option<&str>,
    ) {
        self.add_attribute("http.method".to_string(), serde_json::json!(method));
        self.add_attribute("http.url".to_string(), serde_json::json!(url));
        self.add_attribute("http.target".to_string(), serde_json::json!(target));
        self.add_attribute("http.host".to_string(), serde_json::json!(host));
        self.add_attribute("http.scheme".to_string(), serde_json::json!(scheme));

        if let Some(code) = status_code {
            self.add_attribute("http.status_code".to_string(), serde_json::json!(code));
        }

        if let Some(ua) = user_agent {
            self.add_attribute("http.user_agent".to_string(), serde_json::json!(ua));
        }
    }

    // Set database attributes
    pub fn set_database_attributes(
        &mut self,
        system: &str,
        connection_string: Option<&str>,
        user: Option<&str>,
        statement: &str,
        operation: &str,
        table: Option<&str>,
    ) {
        self.add_attribute("db.system".to_string(), serde_json::json!(system));
        self.add_attribute("db.statement".to_string(), serde_json::json!(statement));
        self.add_attribute("db.operation".to_string(), serde_json::json!(operation));

        if let Some(conn) = connection_string {
            // Sanitize connection string (remove password)
            let sanitized = conn.split('@').next().unwrap_or(conn);
            self.add_attribute("db.connection_string".to_string(), serde_json::json!(sanitized));
        }

        if let Some(u) = user {
            self.add_attribute("db.user".to_string(), serde_json::json!(u));
        }

        if let Some(t) = table {
            self.add_attribute("db.table".to_string(), serde_json::json!(t));
        }
    }

    // Set messaging attributes
    pub fn set_messaging_attributes(
        &mut self,
        system: &str,
        destination: &str,
        destination_kind: &str,
        temp_destination: bool,
        protocol: Option<&str>,
        protocol_version: Option<&str>,
        url: Option<&str>,
        message_id: Option<&str>,
        conversation_id: Option<&str>,
        message_payload_size: Option<usize>,
    ) {
        self.add_attribute("messaging.system".to_string(), serde_json::json!(system));
        self.add_attribute("messaging.destination".to_string(), serde_json::json!(destination));
        self.add_attribute("messaging.destination_kind".to_string(), serde_json::json!(destination_kind));
        self.add_attribute("messaging.temp_destination".to_string(), serde_json::json!(temp_destination));

        if let Some(p) = protocol {
            self.add_attribute("messaging.protocol".to_string(), serde_json::json!(p));
        }

        if let Some(pv) = protocol_version {
            self.add_attribute("messaging.protocol_version".to_string(), serde_json::json!(pv));
        }

        if let Some(u) = url {
            self.add_attribute("messaging.url".to_string(), serde_json::json!(u));
        }

        if let Some(mid) = message_id {
            self.add_attribute("messaging.message_id".to_string(), serde_json::json!(mid));
        }

        if let Some(cid) = conversation_id {
            self.add_attribute("messaging.conversation_id".to_string(), serde_json::json!(cid));
        }

        if let Some(size) = message_payload_size {
            self.add_attribute("messaging.message_payload_size_bytes".to_string(), serde_json::json!(size));
        }
    }

    // Set error attributes
    pub fn set_error_attributes(&mut self, error_type: &str, message: &str, stacktrace: Option<&str>) {
        self.add_attribute("error".to_string(), serde_json::json!(true));
        self.add_attribute("error.type".to_string(), serde_json::json!(error_type));
        self.add_attribute("error.message".to_string(), serde_json::json!(message));

        if let Some(st) = stacktrace {
            self.add_attribute("error.stacktrace".to_string(), serde_json::json!(st));
        }
    }
}