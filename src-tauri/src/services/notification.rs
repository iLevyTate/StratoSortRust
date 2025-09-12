use crate::error::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub sound: Option<String>,
    pub urgency: NotificationUrgency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

pub struct NotificationService {
    app_handle: AppHandle,
}

impl NotificationService {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub async fn send(&self, title: &str, body: &str) -> Result<String> {
        let notification = Notification {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            body: body.to_string(),
            icon: None,
            sound: None,
            urgency: NotificationUrgency::Normal,
        };

        self.send_notification(notification).await
    }

    pub async fn send_notification(&self, notification: Notification) -> Result<String> {
        let id = notification.id.clone();

        // Use Tauri's notification plugin
        match tauri_plugin_notification::NotificationExt::notification(&self.app_handle)
            .builder()
            .title(&notification.title)
            .body(&notification.body)
            .show()
        {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Failed to show system notification: {}", e);
                // Continue anyway, we'll still emit to frontend
            }
        }

        // Also emit to frontend
        self.app_handle.emit("notification", &notification)?;

        Ok(id)
    }

    pub async fn send_success(&self, title: &str, body: &str) -> Result<String> {
        self.send(title, body).await
    }

    pub async fn send_error(&self, title: &str, body: &str) -> Result<String> {
        let notification = Notification {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            body: body.to_string(),
            icon: None,
            sound: None,
            urgency: NotificationUrgency::Critical,
        };

        self.send_notification(notification).await
    }

    pub async fn send_progress(&self, title: &str, progress: f32) -> Result<String> {
        let body = format!("Progress: {:.0}%", progress * 100.0);
        self.send(title, &body).await
    }
}
