use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, warn};

/// WebSocket streaming handler for real-time AI responses
pub struct StreamingHandler {
    app_handle: AppHandle,
    active_streams: Arc<RwLock<Vec<StreamConnection>>>,
}

#[derive(Clone)]
struct StreamConnection {
    id: String,
    #[allow(dead_code)] // Used for future streaming implementation
    sender: mpsc::UnboundedSender<StreamMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    pub content: String,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub stream_id: String,
    pub event_type: StreamEventType,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    Start,
    Data,
    End,
    Error,
}

impl StreamingHandler {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            active_streams: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start a streaming session for AI responses
    pub async fn start_stream(
        &self,
        stream_id: String,
        endpoint: String,
        request_body: serde_json::Value,
    ) -> Result<()> {
        // Create channel for stream messages
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamMessage>();

        // Register the stream
        {
            let mut streams = self.active_streams.write().await;
            streams.push(StreamConnection {
                id: stream_id.clone(),
                sender: tx.clone(),
            });
        }

        // Emit start event
        self.emit_stream_event(StreamEvent {
            stream_id: stream_id.clone(),
            event_type: StreamEventType::Start,
            content: None,
            error: None,
        });

        let app_handle = self.app_handle.clone();
        let stream_id_clone = stream_id.clone();
        let active_streams = self.active_streams.clone();

        // Spawn the WebSocket connection handler
        tokio::spawn(async move {
            match Self::handle_websocket_stream(
                stream_id_clone.clone(),
                endpoint,
                request_body,
                tx,
            )
            .await
            {
                Ok(_) => {
                    debug!("Stream {} completed successfully", stream_id_clone);
                }
                Err(e) => {
                    error!("Stream {} failed: {}", stream_id_clone, e);
                    let _ = app_handle.emit(
                        "ai-stream",
                        StreamEvent {
                            stream_id: stream_id_clone.clone(),
                            event_type: StreamEventType::Error,
                            content: None,
                            error: Some(e.to_string()),
                        },
                    );
                }
            }

            // Clean up the stream
            let mut streams = active_streams.write().await;
            streams.retain(|s| s.id != stream_id_clone);
        });

        // Handle incoming messages and emit to frontend
        let app_handle = self.app_handle.clone();
        let stream_id_clone = stream_id.clone();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let event = if msg.done {
                    StreamEvent {
                        stream_id: stream_id_clone.clone(),
                        event_type: StreamEventType::End,
                        content: Some(msg.content),
                        error: msg.error,
                    }
                } else {
                    StreamEvent {
                        stream_id: stream_id_clone.clone(),
                        event_type: StreamEventType::Data,
                        content: Some(msg.content),
                        error: msg.error,
                    }
                };

                let _ = app_handle.emit("ai-stream", &event);

                if msg.done {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Handle WebSocket connection and streaming
    async fn handle_websocket_stream(
        stream_id: String,
        endpoint: String,
        request_body: serde_json::Value,
        sender: mpsc::UnboundedSender<StreamMessage>,
    ) -> Result<()> {
        // For Ollama, we'll use HTTP streaming instead of WebSocket
        // as Ollama doesn't support WebSocket natively
        Self::handle_http_stream(stream_id, endpoint, request_body, sender).await
    }

    /// Handle HTTP streaming for Ollama
    async fn handle_http_stream(
        stream_id: String,
        endpoint: String,
        mut request_body: serde_json::Value,
        sender: mpsc::UnboundedSender<StreamMessage>,
    ) -> Result<()> {
        // Ensure streaming is enabled
        request_body["stream"] = serde_json::json!(true);

        let client = reqwest::Client::new();
        let response = client
            .post(&endpoint)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AppError::NetworkError {
                message: format!("Failed to connect to Ollama: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(AppError::AiError {
                message: format!("Ollama returned error: {}", response.status()),
            });
        }

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Convert bytes to string and append to buffer
                    let text = String::from_utf8_lossy(&chunk);
                    buffer.push_str(&text);

                    // Process complete lines
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer.drain(..=newline_pos).collect::<String>();
                        let line = line.trim();

                        if line.is_empty() {
                            continue;
                        }

                        // Parse JSON response
                        match serde_json::from_str::<serde_json::Value>(line) {
                            Ok(json) => {
                                let content = json["response"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();

                                let done = json["done"].as_bool().unwrap_or(false);

                                let msg = StreamMessage {
                                    id: stream_id.clone(),
                                    content,
                                    done,
                                    error: None,
                                };

                                if sender.send(msg).is_err() {
                                    debug!("Stream {} receiver dropped", stream_id);
                                    return Ok(());
                                }

                                if done {
                                    return Ok(());
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse streaming response: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    let _ = sender.send(StreamMessage {
                        id: stream_id.clone(),
                        content: String::new(),
                        done: true,
                        error: Some(e.to_string()),
                    });
                    return Err(AppError::NetworkError {
                        message: format!("Stream error: {}", e),
                    });
                }
            }
        }

        // Send final message
        let _ = sender.send(StreamMessage {
            id: stream_id,
            content: String::new(),
            done: true,
            error: None,
        });

        Ok(())
    }

    /// Stop an active stream
    pub async fn stop_stream(&self, stream_id: &str) -> Result<()> {
        let mut streams = self.active_streams.write().await;
        streams.retain(|s| s.id != stream_id);

        self.emit_stream_event(StreamEvent {
            stream_id: stream_id.to_string(),
            event_type: StreamEventType::End,
            content: None,
            error: Some("Stream stopped by user".to_string()),
        });

        Ok(())
    }

    /// Check if a stream is active
    pub async fn is_stream_active(&self, stream_id: &str) -> bool {
        let streams = self.active_streams.read().await;
        streams.iter().any(|s| s.id == stream_id)
    }

    /// Get all active stream IDs
    pub async fn get_active_streams(&self) -> Vec<String> {
        let streams = self.active_streams.read().await;
        streams.iter().map(|s| s.id.clone()).collect()
    }

    /// Emit stream event to frontend
    fn emit_stream_event(&self, event: StreamEvent) {
        let _ = self.app_handle.emit("ai-stream", &event);
    }
}

/// Command to start streaming AI response
#[tauri::command]
pub async fn start_ai_stream(
    stream_id: String,
    prompt: String,
    model: String,
    app_handle: AppHandle,
) -> Result<()> {
    let handler = StreamingHandler::new(app_handle);

    // Construct Ollama endpoint using environment variable or default
    let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let endpoint = format!("{}/api/generate", ollama_host.trim_end_matches('/'));

    let request_body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": true,
    });

    handler.start_stream(stream_id, endpoint, request_body).await
}

/// Command to stop streaming AI response
#[tauri::command]
pub async fn stop_ai_stream(
    stream_id: String,
    app_handle: AppHandle,
) -> Result<()> {
    let handler = StreamingHandler::new(app_handle);
    handler.stop_stream(&stream_id).await
}

/// Command to check if stream is active
#[tauri::command]
pub async fn is_stream_active(
    stream_id: String,
    app_handle: AppHandle,
) -> Result<bool> {
    let handler = StreamingHandler::new(app_handle);
    Ok(handler.is_stream_active(&stream_id).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_message_serialization() {
        let msg = StreamMessage {
            id: "test".to_string(),
            content: "Hello".to_string(),
            done: false,
            error: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"done\":false"));
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent {
            stream_id: "test".to_string(),
            event_type: StreamEventType::Data,
            content: Some("Hello".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"stream_id\":\"test\""));
        assert!(json.contains("\"event_type\":\"data\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }
}