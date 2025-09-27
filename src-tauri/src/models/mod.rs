// Data Models Module
// Provides core data structures for the application

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// File model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub size: i64,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub smart_folder_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed: Option<DateTime<Utc>>,
    pub is_favorite: bool,
    pub is_archived: bool,
}

// Tag model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Smart folder model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartFolder {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub rules: serde_json::Value,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub parent_id: Option<i64>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}