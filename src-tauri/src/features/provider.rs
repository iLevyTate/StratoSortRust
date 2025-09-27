// Feature Flag Provider
// Sources and management for feature flags

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::warn;

use super::flags::{FeatureFlag, FlagCollection, FlagValue};
use crate::error::AppError;

// Flag source types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlagSource {
    Local,
    Remote,
    File,
    Database,
    Environment,
}

// Flag provider trait
#[async_trait]
pub trait FlagProvider: Send + Sync {
    // Load flags from source
    async fn load(&self) -> Result<FlagCollection, AppError>;

    // Save flags to source
    async fn save(&self, collection: &FlagCollection) -> Result<(), AppError>;

    // Get source type
    fn source_type(&self) -> FlagSource;

    // Check if source is available
    async fn is_available(&self) -> bool;
}

// Local memory provider
pub struct LocalProvider {
    flags: Arc<RwLock<FlagCollection>>,
}

impl Default for LocalProvider {
    fn default() -> Self {
        Self {
            flags: Arc::new(RwLock::new(FlagCollection::new("local".to_string()))),
        }
    }
}

impl LocalProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_flags(flags: FlagCollection) -> Self {
        Self {
            flags: Arc::new(RwLock::new(flags)),
        }
    }
}

#[async_trait]
impl FlagProvider for LocalProvider {
    async fn load(&self) -> Result<FlagCollection, AppError> {
        Ok(self.flags.read().await.clone())
    }

    async fn save(&self, collection: &FlagCollection) -> Result<(), AppError> {
        *self.flags.write().await = collection.clone();
        Ok(())
    }

    fn source_type(&self) -> FlagSource {
        FlagSource::Local
    }

    async fn is_available(&self) -> bool {
        true
    }
}

// File-based provider
pub struct FileProvider {
    file_path: PathBuf,
}

impl FileProvider {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }
}

#[async_trait]
impl FlagProvider for FileProvider {
    async fn load(&self) -> Result<FlagCollection, AppError> {
        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| AppError::IoError {
                message: format!("Failed to read flag file: {}", e),
            })?;

        serde_json::from_str(&content)
            .map_err(|e| AppError::ParseError {
                message: format!("Failed to parse flag file: {}", e),
            })
    }

    async fn save(&self, collection: &FlagCollection) -> Result<(), AppError> {
        let content = serde_json::to_string_pretty(collection)
            .map_err(|e| AppError::SerializationError {
                message: format!("Failed to serialize flags: {}", e),
            })?;

        // Create parent directory if needed
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::IoError {
                    message: format!("Failed to create directory: {}", e),
                })?;
        }

        tokio::fs::write(&self.file_path, content)
            .await
            .map_err(|e| AppError::IoError {
                message: format!("Failed to write flag file: {}", e),
            })?;

        Ok(())
    }

    fn source_type(&self) -> FlagSource {
        FlagSource::File
    }

    async fn is_available(&self) -> bool {
        self.file_path.exists()
    }
}

// Environment variable provider
pub struct EnvProvider {
    prefix: String,
}

impl EnvProvider {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

#[async_trait]
impl FlagProvider for EnvProvider {
    async fn load(&self) -> Result<FlagCollection, AppError> {
        let mut collection = FlagCollection::new("environment".to_string());

        // Read environment variables with prefix
        for (key, value) in std::env::vars() {
            if key.starts_with(&self.prefix) {
                let flag_key = key.strip_prefix(&self.prefix)
                    .unwrap()
                    .to_lowercase()
                    .replace('_', ".");

                // Parse value
                let flag_value = if value == "true" || value == "1" {
                    FlagValue::Boolean(true)
                } else if value == "false" || value == "0" {
                    FlagValue::Boolean(false)
                } else if let Ok(i) = value.parse::<i64>() {
                    FlagValue::Integer(i)
                } else if let Ok(f) = value.parse::<f64>() {
                    FlagValue::Float(f)
                } else {
                    FlagValue::String(value)
                };

                let flag = FeatureFlag::boolean(flag_key.clone(), false);
                let mut flag = flag;
                flag.default_value = flag_value;

                collection.add_flag(flag);
            }
        }

        Ok(collection)
    }

    async fn save(&self, _collection: &FlagCollection) -> Result<(), AppError> {
        // Environment variables are read-only
        Err(AppError::InvalidOperation {
            message: "Cannot save to environment variables".to_string(),
        })
    }

    fn source_type(&self) -> FlagSource {
        FlagSource::Environment
    }

    async fn is_available(&self) -> bool {
        true
    }
}

// Remote provider (HTTP)
pub struct RemoteProvider {
    endpoint: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl RemoteProvider {
    pub fn new(endpoint: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            endpoint,
            api_key,
            client,
        }
    }
}

#[async_trait]
impl FlagProvider for RemoteProvider {
    async fn load(&self) -> Result<FlagCollection, AppError> {
        let mut request = self.client.get(&self.endpoint);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::NetworkError {
                message: format!("Failed to fetch flags: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError {
                service: "Feature Flag Service".to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        response
            .json::<FlagCollection>()
            .await
            .map_err(|e| AppError::ParseError {
                message: format!("Failed to parse flag response: {}", e),
            })
    }

    async fn save(&self, collection: &FlagCollection) -> Result<(), AppError> {
        let mut request = self.client
            .put(&self.endpoint)
            .json(collection);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::NetworkError {
                message: format!("Failed to save flags: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError {
                service: "Feature Flag Service".to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        Ok(())
    }

    fn source_type(&self) -> FlagSource {
        FlagSource::Remote
    }

    async fn is_available(&self) -> bool {
        // Check if endpoint is reachable
        let result = self.client
            .head(&self.endpoint)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await;

        result.is_ok()
    }
}

// Multi-source provider with fallback
pub struct MultiProvider {
    providers: Vec<Box<dyn FlagProvider>>,
    merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    FirstWins,  // Use first available source
    LastWins,   // Override with later sources
    Merge,      // Merge all sources
}

impl MultiProvider {
    pub fn new(merge_strategy: MergeStrategy) -> Self {
        Self {
            providers: Vec::new(),
            merge_strategy,
        }
    }

    pub fn add_provider(mut self, provider: Box<dyn FlagProvider>) -> Self {
        self.providers.push(provider);
        self
    }
}

#[async_trait]
impl FlagProvider for MultiProvider {
    async fn load(&self) -> Result<FlagCollection, AppError> {
        let mut combined = FlagCollection::new("multi".to_string());

        match self.merge_strategy {
            MergeStrategy::FirstWins => {
                // Use first available provider
                for provider in &self.providers {
                    if provider.is_available().await {
                        match provider.load().await {
                            Ok(collection) => return Ok(collection),
                            Err(e) => {
                                warn!(
                                    source = ?provider.source_type(),
                                    error = %e,
                                    "Failed to load from provider"
                                );
                            }
                        }
                    }
                }
            }

            MergeStrategy::LastWins => {
                // Load from all providers, later ones override
                for provider in &self.providers {
                    if provider.is_available().await {
                        match provider.load().await {
                            Ok(collection) => {
                                for (key, flag) in collection.flags {
                                    combined.flags.insert(key, flag);
                                }
                            }
                            Err(e) => {
                                warn!(
                                    source = ?provider.source_type(),
                                    error = %e,
                                    "Failed to load from provider"
                                );
                            }
                        }
                    }
                }
            }

            MergeStrategy::Merge => {
                // Merge all sources
                let mut all_flags = HashMap::new();

                for provider in &self.providers {
                    if provider.is_available().await {
                        match provider.load().await {
                            Ok(collection) => {
                                for (key, flag) in collection.flags {
                                    all_flags.entry(key)
                                        .or_insert_with(Vec::new)
                                        .push(flag);
                                }
                            }
                            Err(e) => {
                                warn!(
                                    source = ?provider.source_type(),
                                    error = %e,
                                    "Failed to load from provider"
                                );
                            }
                        }
                    }
                }

                // Merge flags with same key
                for (key, flags) in all_flags {
                    if let Some(merged) = Self::merge_flags(flags) {
                        combined.flags.insert(key, merged);
                    }
                }
            }
        }

        if combined.flags.is_empty() {
            return Err(AppError::NotFound {
                message: "No flags loaded from any provider".to_string(),
            });
        }

        Ok(combined)
    }

    async fn save(&self, collection: &FlagCollection) -> Result<(), AppError> {
        // Save to all writable providers
        let mut errors = Vec::new();

        for provider in &self.providers {
            if let Err(e) = provider.save(collection).await {
                errors.push(format!("{:?}: {}", provider.source_type(), e));
            }
        }

        if !errors.is_empty() {
            return Err(AppError::OperationError {
                message: format!("Failed to save to some providers: {}", errors.join(", ")),
            });
        }

        Ok(())
    }

    fn source_type(&self) -> FlagSource {
        FlagSource::Local // Multi-provider appears as local
    }

    async fn is_available(&self) -> bool {
        // At least one provider is available
        for provider in &self.providers {
            if provider.is_available().await {
                return true;
            }
        }
        false
    }
}

impl MultiProvider {
    // Merge multiple flag definitions
    fn merge_flags(flags: Vec<FeatureFlag>) -> Option<FeatureFlag> {
        if flags.is_empty() {
            return None;
        }

        let mut merged = flags[0].clone();

        for flag in flags.iter().skip(1) {
            // Merge rules
            for rule in &flag.rules {
                if !merged.rules.iter().any(|r| r.name == rule.name) {
                    merged.rules.push(rule.clone());
                }
            }

            // Merge variants
            for (key, value) in &flag.variants {
                merged.variants.entry(key.clone()).or_insert(value.clone());
            }

            // Merge tags
            for tag in &flag.tags {
                if !merged.tags.contains(tag) {
                    merged.tags.push(tag.clone());
                }
            }

            // Use most recent update time
            if flag.updated_at > merged.updated_at {
                merged.updated_at = flag.updated_at;
            }
        }

        Some(merged)
    }
}