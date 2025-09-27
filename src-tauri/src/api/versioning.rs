// API Versioning System
// Provides version management and backward compatibility for API endpoints

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;
use tokio::sync::RwLock;

use crate::error::AppError;

// API version structure
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

// Implement Display trait for ApiVersion
impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl ApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    pub fn from_string(version: &str) -> Result<Self, AppError> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(AppError::InvalidInput {
                message: format!("Invalid version format: {}", version),
            });
        }

        Ok(Self {
            major: parts[0].parse().map_err(|_| AppError::InvalidInput {
                message: "Invalid major version".to_string(),
            })?,
            minor: parts[1].parse().map_err(|_| AppError::InvalidInput {
                message: "Invalid minor version".to_string(),
            })?,
            patch: parts[2].parse().map_err(|_| AppError::InvalidInput {
                message: "Invalid patch version".to_string(),
            })?,
        })
    }


    // Check if this version is compatible with another version
    pub fn is_compatible_with(&self, other: &ApiVersion) -> bool {
        // Major version must match for compatibility
        if self.major != other.major {
            return false;
        }

        // Minor version must be greater or equal (backward compatible)
        if self.minor < other.minor {
            return false;
        }

        // Patch version doesn't affect compatibility
        true
    }

    // Check if this version is newer than another
    pub fn is_newer_than(&self, other: &ApiVersion) -> bool {
        if self.major > other.major {
            return true;
        }
        if self.major < other.major {
            return false;
        }

        if self.minor > other.minor {
            return true;
        }
        if self.minor < other.minor {
            return false;
        }

        self.patch > other.patch
    }
}

// API endpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub path: String,
    pub method: HttpMethod,
    pub handler: String,
    pub introduced_in: ApiVersion,
    pub deprecated_in: Option<ApiVersion>,
    pub removed_in: Option<ApiVersion>,
    pub description: String,
    pub request_schema: Option<serde_json::Value>,
    pub response_schema: Option<serde_json::Value>,
    pub breaking_changes: Vec<BreakingChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

// Breaking change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChange {
    pub version: ApiVersion,
    pub description: String,
    pub migration_guide: String,
}

// API transformation rule
// Note: Cannot derive Clone or Debug due to Box<dyn Fn> closures
pub struct ApiTransformation {
    pub from_version: ApiVersion,
    pub to_version: ApiVersion,
    pub transform_request: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, AppError> + Send + Sync>,
    pub transform_response: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, AppError> + Send + Sync>,
}

// Manual Debug implementation for ApiTransformation
impl std::fmt::Debug for ApiTransformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiTransformation")
            .field("from_version", &self.from_version)
            .field("to_version", &self.to_version)
            .field("transform_request", &"<closure>")
            .field("transform_response", &"<closure>")
            .finish()
    }
}

// Version migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMigration {
    pub from_version: ApiVersion,
    pub to_version: ApiVersion,
    pub changes: Vec<ApiChange>,
    pub migration_date: DateTime<Utc>,
    pub is_breaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiChange {
    EndpointAdded { path: String, method: String },
    EndpointRemoved { path: String, method: String },
    EndpointModified { path: String, method: String, changes: Vec<String> },
    ParameterAdded { endpoint: String, parameter: String, required: bool },
    ParameterRemoved { endpoint: String, parameter: String },
    ParameterTypeChanged { endpoint: String, parameter: String, old_type: String, new_type: String },
    ResponseFormatChanged { endpoint: String, description: String },
}

// API version manager
pub struct ApiVersionManager {
    current_version: ApiVersion,
    supported_versions: Arc<RwLock<Vec<ApiVersion>>>,
    endpoints: Arc<RwLock<HashMap<ApiVersion, Vec<ApiEndpoint>>>>,
    transformations: Arc<RwLock<Vec<ApiTransformation>>>,
    migrations: Arc<RwLock<Vec<VersionMigration>>>,
    deprecation_policy: DeprecationPolicy,
}

#[derive(Debug, Clone)]
pub struct DeprecationPolicy {
    pub deprecation_period_days: u32,
    pub min_supported_version: ApiVersion,
    pub max_supported_versions: usize,
}

impl Default for DeprecationPolicy {
    fn default() -> Self {
        Self {
            deprecation_period_days: 180, // 6 months
            min_supported_version: ApiVersion::new(1, 0, 0),
            max_supported_versions: 3, // Support last 3 major versions
        }
    }
}

impl ApiVersionManager {
    pub fn new(current_version: ApiVersion) -> Self {
        let current_version_clone = current_version.clone();
        Self {
            current_version,
            supported_versions: Arc::new(RwLock::new(vec![current_version_clone])),
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            transformations: Arc::new(RwLock::new(Vec::new())),
            migrations: Arc::new(RwLock::new(Vec::new())),
            deprecation_policy: DeprecationPolicy::default(),
        }
    }

    // Register an API endpoint
    pub async fn register_endpoint(&self, version: ApiVersion, endpoint: ApiEndpoint) {
        let mut endpoints = self.endpoints.write().await;
        endpoints
            .entry(version)
            .or_insert_with(Vec::new)
            .push(endpoint);
    }

    // Register a version transformation
    pub async fn register_transformation(
        &self,
        from: ApiVersion,
        to: ApiVersion,
        transform_request: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, AppError> + Send + Sync>,
        transform_response: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, AppError> + Send + Sync>,
    ) {
        let mut transformations = self.transformations.write().await;
        transformations.push(ApiTransformation {
            from_version: from,
            to_version: to,
            transform_request,
            transform_response,
        });
    }

    // Get endpoints for a specific version
    pub async fn get_endpoints(&self, version: &ApiVersion) -> Vec<ApiEndpoint> {
        let endpoints = self.endpoints.read().await;

        // If exact version exists, return it
        if let Some(exact) = endpoints.get(version) {
            return exact.clone();
        }

        // Find the closest compatible version
        let mut compatible_endpoints = Vec::new();
        for (v, eps) in endpoints.iter() {
            if version.is_compatible_with(v) {
                for endpoint in eps {
                    // Skip deprecated/removed endpoints
                    if let Some(removed) = &endpoint.removed_in {
                        if version.is_newer_than(removed) || version == removed {
                            continue;
                        }
                    }
                    compatible_endpoints.push(endpoint.clone());
                }
            }
        }

        compatible_endpoints
    }

    // Transform request from client version to current version
    pub async fn transform_request(
        &self,
        from_version: &ApiVersion,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        if from_version == &self.current_version {
            return Ok(request);
        }

        let transformations = self.transformations.read().await;

        // Find transformation path
        let path = self.find_transformation_path(from_version, &self.current_version, &transformations)?;

        // Apply transformations in sequence
        let mut transformed = request;
        for transformation in path {
            transformed = (transformation.transform_request)(transformed)?;
        }

        Ok(transformed)
    }

    // Transform response from current version to client version
    pub async fn transform_response(
        &self,
        to_version: &ApiVersion,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        if to_version == &self.current_version {
            return Ok(response);
        }

        let transformations = self.transformations.read().await;

        // Find transformation path (reverse)
        let path = self.find_transformation_path(&self.current_version, to_version, &transformations)?;

        // Apply transformations in reverse sequence
        let mut transformed = response;
        for transformation in path.iter().rev() {
            transformed = (transformation.transform_response)(transformed)?;
        }

        Ok(transformed)
    }

    // Find transformation path between versions
    fn find_transformation_path<'a>(
        &self,
        from: &ApiVersion,
        to: &ApiVersion,
        transformations: &'a [ApiTransformation],
    ) -> Result<Vec<&'a ApiTransformation>, AppError> {
        // Simple linear search for now
        // In production, use graph algorithms for complex version trees

        let mut path = Vec::new();
        let mut current = from.clone();

        while current != *to {
            let mut found = false;

            for transformation in transformations {
                if transformation.from_version == current {
                    path.push(transformation);
                    current = transformation.to_version.clone();
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(AppError::InvalidInput {
                    message: format!(
                        "No transformation path from version {} to {}",
                        from,
                        to
                    ),
                });
            }

            // Prevent infinite loops
            if path.len() > 10 {
                return Err(AppError::InvalidInput {
                    message: "Transformation path too long".to_string(),
                });
            }
        }

        Ok(path)
    }

    // Check if a version is supported
    pub async fn is_version_supported(&self, version: &ApiVersion) -> bool {
        let supported = self.supported_versions.read().await;
        supported.iter().any(|v| v.is_compatible_with(version))
    }

    // Add a new version
    pub async fn add_version(&self, version: ApiVersion, migration: VersionMigration) {
        let mut supported = self.supported_versions.write().await;
        let mut migrations = self.migrations.write().await;

        supported.push(version.clone());
        migrations.push(migration);

        // Apply deprecation policy
        self.apply_deprecation_policy(&mut supported).await;
    }

    // Apply deprecation policy
    async fn apply_deprecation_policy(&self, supported: &mut Vec<ApiVersion>) {
        // Sort versions
        supported.sort_by(|a, b| {
            if a.is_newer_than(b) {
                std::cmp::Ordering::Greater
            } else if b.is_newer_than(a) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        });

        // Keep only the allowed number of versions
        if supported.len() > self.deprecation_policy.max_supported_versions {
            let to_remove = supported.len() - self.deprecation_policy.max_supported_versions;
            supported.drain(0..to_remove);
        }

        // Ensure minimum version is supported
        supported.retain(|v| !self.deprecation_policy.min_supported_version.is_newer_than(v));
    }

    // Get deprecation warnings for a version
    pub async fn get_deprecation_warnings(&self, version: &ApiVersion) -> Vec<String> {
        let mut warnings = Vec::new();

        let endpoints = self.endpoints.read().await;
        let supported = self.supported_versions.read().await;

        // Check if version is deprecated
        if let Some(newest) = supported.last() {
            if newest.is_newer_than(version) {
                warnings.push(format!(
                    "API version {} is deprecated. Please upgrade to {}",
                    version, // Uses Display trait automatically in format!
                    newest  // Uses Display trait automatically in format!
                ));
            }
        }

        // Check for deprecated endpoints
        if let Some(version_endpoints) = endpoints.get(version) {
            for endpoint in version_endpoints {
                if endpoint.deprecated_in.is_some() {
                    warnings.push(format!(
                        "Endpoint {} {} is deprecated",
                        endpoint.method.as_str(),
                        endpoint.path
                    ));
                }
            }
        }

        warnings
    }

    // Generate API documentation for a version
    pub async fn generate_documentation(&self, version: &ApiVersion) -> ApiDocumentation {
        let endpoints = self.get_endpoints(version).await;
        let warnings = self.get_deprecation_warnings(version).await;
        let migrations = self.migrations.read().await;

        let relevant_migrations: Vec<VersionMigration> = migrations
            .iter()
            .filter(|m| m.to_version == *version || m.from_version == *version)
            .cloned()
            .collect();

        ApiDocumentation {
            version: version.clone(),
            generated_at: Utc::now(),
            endpoints,
            deprecation_warnings: warnings,
            migrations: relevant_migrations,
            base_url: "/api".to_string(),
        }
    }

    // Get version from request header
    pub fn get_version_from_header(headers: &HashMap<String, String>) -> Result<ApiVersion, AppError> {
        headers
            .get("api-version")
            .or_else(|| headers.get("x-api-version"))
            .map(|v| ApiVersion::from_string(v))
            .unwrap_or_else(|| Ok(ApiVersion::new(1, 0, 0))) // Default to v1.0.0
    }
}

// API documentation
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiDocumentation {
    pub version: ApiVersion,
    pub generated_at: DateTime<Utc>,
    pub endpoints: Vec<ApiEndpoint>,
    pub deprecation_warnings: Vec<String>,
    pub migrations: Vec<VersionMigration>,
    pub base_url: String,
}

impl HttpMethod {
    // Renamed from to_string() to avoid conflict with ToString trait
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
        }
    }
}

// Version compatibility matrix
#[derive(Debug, Serialize, Deserialize)]
pub struct CompatibilityMatrix {
    pub versions: Vec<ApiVersion>,
    pub compatibility: HashMap<(ApiVersion, ApiVersion), bool>,
}

impl CompatibilityMatrix {
    pub fn new(versions: Vec<ApiVersion>) -> Self {
        let mut compatibility = HashMap::new();

        for v1 in &versions {
            for v2 in &versions {
                compatibility.insert((v1.clone(), v2.clone()), v1.is_compatible_with(v2));
            }
        }

        Self {
            versions,
            compatibility,
        }
    }

    pub fn is_compatible(&self, v1: &ApiVersion, v2: &ApiVersion) -> bool {
        self.compatibility
            .get(&(v1.clone(), v2.clone()))
            .copied()
            .unwrap_or(false)
    }
}

// Note: Types are already public, no need for re-export