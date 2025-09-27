// Trace Context Management
// Context propagation for distributed tracing

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use super::tracing::{TraceId, SpanId};

// Trace context for propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: Option<SpanId>,
    pub sampled: bool,
    pub trace_state: Option<String>,
    pub baggage: Baggage,
}

impl TraceContext {
    // Create new trace context
    pub fn new(trace_id: TraceId, span_id: Option<SpanId>, sampled: bool) -> Self {
        Self {
            trace_id,
            span_id,
            sampled,
            trace_state: None,
            baggage: Baggage::new(),
        }
    }

    // Create child context
    pub fn create_child(&self, span_id: SpanId) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Some(span_id),
            sampled: self.sampled,
            trace_state: self.trace_state.clone(),
            baggage: self.baggage.clone(),
        }
    }

    // Check if context is valid
    pub fn is_valid(&self) -> bool {
        // Trace ID should not be all zeros
        !self.trace_id.as_str().chars().all(|c| c == '0')
    }

    // Get parent span ID
    pub fn parent_span_id(&self) -> Option<&SpanId> {
        self.span_id.as_ref()
    }
}

// Baggage for context propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baggage {
    items: HashMap<String, BaggageItem>,
}

impl Baggage {
    // Create new baggage
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    // Set baggage item
    pub fn set(&mut self, key: String, value: String, metadata: Option<String>) {
        self.items.insert(key.clone(), BaggageItem {
            key,
            value,
            metadata,
        });
    }

    // Get baggage item
    pub fn get(&self, key: &str) -> Option<&BaggageItem> {
        self.items.get(key)
    }

    // Remove baggage item
    pub fn remove(&mut self, key: &str) -> Option<BaggageItem> {
        self.items.remove(key)
    }

    // Get all items
    pub fn items(&self) -> &HashMap<String, BaggageItem> {
        &self.items
    }

    // Clear all items
    pub fn clear(&mut self) {
        self.items.clear();
    }

    // Serialize to header string (W3C Baggage format)
    pub fn to_header_string(&self) -> String {
        self.items
            .values()
            .map(|item| {
                if let Some(ref metadata) = item.metadata {
                    format!("{}={};{}", item.key, item.value, metadata)
                } else {
                    format!("{}={}", item.key, item.value)
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    // Parse from header string
    pub fn from_header_string(header: &str) -> Self {
        let mut baggage = Self::new();

        for item_str in header.split(',') {
            let item_str = item_str.trim();
            if item_str.is_empty() {
                continue;
            }

            // Parse key=value;metadata format
            let parts: Vec<&str> = item_str.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].trim().to_string();
            let value_and_metadata = parts[1];

            // Check for metadata
            let (value, metadata) = if let Some(semi_idx) = value_and_metadata.find(';') {
                let value = value_and_metadata[..semi_idx].trim().to_string();
                let metadata = value_and_metadata[semi_idx + 1..].trim().to_string();
                (value, Some(metadata))
            } else {
                (value_and_metadata.trim().to_string(), None)
            };

            baggage.set(key, value, metadata);
        }

        baggage
    }
}

// Baggage item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaggageItem {
    pub key: String,
    pub value: String,
    pub metadata: Option<String>,
}

// Context carrier for different transport mechanisms
#[derive(Debug, Clone)]
pub enum ContextCarrier {
    HttpHeaders(HashMap<String, String>),
    GrpcMetadata(HashMap<String, Vec<u8>>),
    TextMap(HashMap<String, String>),
    Binary(Vec<u8>),
}

impl ContextCarrier {
    // Get value by key
    pub fn get(&self, key: &str) -> Option<String> {
        match self {
            Self::HttpHeaders(headers) => headers.get(key).cloned(),
            Self::TextMap(map) => map.get(key).cloned(),
            Self::GrpcMetadata(metadata) => {
                metadata.get(key).and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            }
            Self::Binary(_) => None, // Binary format requires special handling
        }
    }

    // Set value by key
    pub fn set(&mut self, key: String, value: String) {
        match self {
            Self::HttpHeaders(headers) => {
                headers.insert(key, value);
            }
            Self::TextMap(map) => {
                map.insert(key, value);
            }
            Self::GrpcMetadata(metadata) => {
                metadata.insert(key, value.into_bytes());
            }
            Self::Binary(_) => {} // Binary format requires special handling
        }
    }

    // Get all keys
    pub fn keys(&self) -> Vec<String> {
        match self {
            Self::HttpHeaders(headers) => headers.keys().cloned().collect(),
            Self::TextMap(map) => map.keys().cloned().collect(),
            Self::GrpcMetadata(metadata) => metadata.keys().cloned().collect(),
            Self::Binary(_) => vec![],
        }
    }
}

// Context manager for thread-local storage
pub struct ContextManager {
    contexts: HashMap<std::thread::ThreadId, TraceContext>,
}

impl ContextManager {
    // Create new context manager
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
        }
    }

    // Set current context for thread
    pub fn set_current(&mut self, context: TraceContext) {
        let thread_id = std::thread::current().id();
        self.contexts.insert(thread_id, context);
    }

    // Get current context for thread
    pub fn get_current(&self) -> Option<&TraceContext> {
        let thread_id = std::thread::current().id();
        self.contexts.get(&thread_id)
    }

    // Remove current context for thread
    pub fn remove_current(&mut self) -> Option<TraceContext> {
        let thread_id = std::thread::current().id();
        self.contexts.remove(&thread_id)
    }

    // Clear all contexts
    pub fn clear(&mut self) {
        self.contexts.clear();
    }
}