use crate::error::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn, error};

/// Rate limiting configuration per endpoint
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Whether to apply per-IP limiting
    pub per_ip: bool,
    /// Burst allowance (temporary spike tolerance)
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 10,
        }
    }
}

/// Endpoint-specific rate limit configurations
pub struct EndpointLimits {
    configs: DashMap<String, RateLimitConfig>,
}

impl Default for EndpointLimits {
    fn default() -> Self {
        let configs = DashMap::new();

        // Critical endpoints with strict limits
        configs.insert("delete_file".to_string(), RateLimitConfig {
            max_requests: 10,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 2,
        });

        configs.insert("batch_delete".to_string(), RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 1,
        });

        // AI endpoints (expensive operations)
        configs.insert("analyze_with_ai".to_string(), RateLimitConfig {
            max_requests: 20,
            window_duration: Duration::from_secs(300), // 5 minutes
            per_ip: true,
            burst_size: 3,
        });

        configs.insert("generate_embeddings".to_string(), RateLimitConfig {
            max_requests: 30,
            window_duration: Duration::from_secs(300),
            per_ip: true,
            burst_size: 5,
        });

        // File operations
        configs.insert("move_file".to_string(), RateLimitConfig {
            max_requests: 50,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 10,
        });

        configs.insert("copy_file".to_string(), RateLimitConfig {
            max_requests: 50,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 10,
        });

        // Read operations (more lenient)
        configs.insert("get_files".to_string(), RateLimitConfig {
            max_requests: 200,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 20,
        });

        configs.insert("get_file_content".to_string(), RateLimitConfig {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 15,
        });

        // History operations
        configs.insert("undo".to_string(), RateLimitConfig {
            max_requests: 30,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 5,
        });

        configs.insert("redo".to_string(), RateLimitConfig {
            max_requests: 30,
            window_duration: Duration::from_secs(60),
            per_ip: true,
            burst_size: 5,
        });

        // Default for unspecified endpoints
        configs.insert("_default".to_string(), RateLimitConfig::default());

        Self { configs }
    }
}

/// Request tracking information
#[derive(Debug, Clone)]
struct RequestInfo {
    /// Number of requests in current window
    count: u32,
    /// Window start time
    window_start: Instant,
    /// Burst tokens available
    burst_tokens: u32,
    /// Last request time
    last_request: Instant,
}

impl RequestInfo {
    fn new(burst_size: u32) -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
            burst_tokens: burst_size,
            last_request: Instant::now(),
        }
    }

    /// Check if request should be allowed
    fn should_allow(&mut self, config: &RateLimitConfig) -> bool {
        let now = Instant::now();

        // Reset window if expired
        if now.duration_since(self.window_start) >= config.window_duration {
            self.window_start = now;
            self.count = 0;
            self.burst_tokens = config.burst_size;
        }

        // Refill burst tokens gradually
        let time_since_last = now.duration_since(self.last_request);
        if time_since_last >= Duration::from_secs(1) {
            let tokens_to_add = (time_since_last.as_secs() as u32).min(config.burst_size);
            self.burst_tokens = (self.burst_tokens + tokens_to_add).min(config.burst_size);
        }

        self.last_request = now;

        // Check if under limit
        if self.count < config.max_requests {
            self.count += 1;
            true
        } else if self.burst_tokens > 0 {
            // Use burst token
            self.burst_tokens -= 1;
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Get time until rate limit resets
    fn time_until_reset(&self, config: &RateLimitConfig) -> Duration {
        let elapsed = Instant::now().duration_since(self.window_start);
        if elapsed < config.window_duration {
            config.window_duration - elapsed
        } else {
            Duration::ZERO
        }
    }
}

/// Rate limiter implementation
pub struct RateLimiter {
    /// Per-client request tracking (key = client_id:endpoint)
    requests: Arc<DashMap<String, RequestInfo>>,
    /// Endpoint configurations
    endpoints: Arc<EndpointLimits>,
    /// Global rate limit (across all clients)
    global_limits: Arc<RwLock<RequestInfo>>,
    /// Enable/disable rate limiting
    enabled: Arc<RwLock<bool>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(DashMap::new()),
            endpoints: Arc::new(EndpointLimits::default()),
            global_limits: Arc::new(RwLock::new(RequestInfo::new(100))),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Check if request should be allowed
    pub async fn check_rate_limit(
        &self,
        endpoint: &str,
        client_id: Option<&str>,
    ) -> Result<RateLimitStatus> {
        // Check if rate limiting is enabled
        if !*self.enabled.read().await {
            return Ok(RateLimitStatus::Allowed);
        }

        // Get endpoint config
        let config = self.endpoints.configs
            .get(endpoint)
            .map(|c| c.clone())
            .unwrap_or_else(|| {
                self.endpoints.configs
                    .get("_default")
                    .map(|c| c.clone())
                    .unwrap_or_default()
            });

        // Build request key
        let key = if config.per_ip && client_id.is_some() {
            format!("{}:{}", client_id.unwrap(), endpoint)
        } else {
            format!("global:{}", endpoint)
        };

        // Check rate limit
        let mut entry = self.requests
            .entry(key.clone())
            .or_insert_with(|| RequestInfo::new(config.burst_size));

        if entry.should_allow(&config) {
            debug!("Rate limit check passed for {}", key);
            Ok(RateLimitStatus::Allowed)
        } else {
            let retry_after = entry.time_until_reset(&config);
            warn!(
                "Rate limit exceeded for {} ({}). Retry after {:?}",
                endpoint, key, retry_after
            );

            // Log potential abuse
            if entry.count > config.max_requests * 2 {
                error!(
                    "Potential abuse detected: {} has made {} requests to {}",
                    client_id.unwrap_or("unknown"),
                    entry.count,
                    endpoint
                );
            }

            Ok(RateLimitStatus::Limited {
                retry_after_seconds: retry_after.as_secs() as u32,
                limit: config.max_requests,
                remaining: 0,
                reset_time_secs: (entry.window_start + config.window_duration).elapsed().as_secs(),
            })
        }
    }

    /// Clean up old entries to prevent memory growth
    pub async fn cleanup_old_entries(&self) {
        let cutoff = Duration::from_secs(3600); // 1 hour
        let mut removed = 0;

        self.requests.retain(|_key, info| {
            let age = Instant::now().duration_since(info.last_request);
            if age > cutoff {
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed > 0 {
            debug!("Cleaned up {} old rate limit entries", removed);
        }
    }

    /// Update configuration for an endpoint
    pub fn update_endpoint_config(&self, endpoint: String, config: RateLimitConfig) {
        self.endpoints.configs.insert(endpoint, config);
    }

    /// Enable or disable rate limiting
    pub async fn set_enabled(&self, enabled: bool) {
        *self.enabled.write().await = enabled;
    }

    /// Get current statistics
    pub fn get_stats(&self) -> RateLimitStats {
        RateLimitStats {
            total_clients: self.requests.len(),
            enabled: self.enabled.blocking_read().clone(),
            endpoints_configured: self.endpoints.configs.len(),
        }
    }
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitStatus {
    Allowed,
    Limited {
        retry_after_seconds: u32,
        limit: u32,
        remaining: u32,
        reset_time_secs: u64,
    },
}

/// Rate limiter statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStats {
    pub total_clients: usize,
    pub enabled: bool,
    pub endpoints_configured: usize,
}

/// Tauri command wrapper with rate limiting
#[macro_export]
macro_rules! rate_limited_command {
    ($limiter:expr, $endpoint:expr, $client_id:expr, $body:expr) => {{
        match $limiter.check_rate_limit($endpoint, $client_id).await? {
            $crate::middleware::rate_limit::RateLimitStatus::Allowed => $body,
            $crate::middleware::rate_limit::RateLimitStatus::Limited { retry_after_seconds, .. } => {
                Err($crate::error::AppError::RateLimitExceeded {
                    retry_after_seconds,
                    endpoint: $endpoint.to_string(),
                })
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting() {
        let limiter = RateLimiter::new();

        // Configure test endpoint
        limiter.update_endpoint_config("test".to_string(), RateLimitConfig {
            max_requests: 3,
            window_duration: Duration::from_secs(1),
            per_ip: true,
            burst_size: 1,
        });

        // Should allow first 3 requests
        for i in 0..3 {
            let status = limiter.check_rate_limit("test", Some("client1")).await.unwrap();
            assert!(matches!(status, RateLimitStatus::Allowed), "Request {} should be allowed", i + 1);
        }

        // 4th request should be allowed (burst)
        let status = limiter.check_rate_limit("test", Some("client1")).await.unwrap();
        assert!(matches!(status, RateLimitStatus::Allowed), "Burst request should be allowed");

        // 5th request should be limited
        let status = limiter.check_rate_limit("test", Some("client1")).await.unwrap();
        assert!(matches!(status, RateLimitStatus::Limited { .. }), "5th request should be limited");

        // Different client should be allowed
        let status = limiter.check_rate_limit("test", Some("client2")).await.unwrap();
        assert!(matches!(status, RateLimitStatus::Allowed), "Different client should be allowed");

        // Wait for window reset
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should allow again after reset
        let status = limiter.check_rate_limit("test", Some("client1")).await.unwrap();
        assert!(matches!(status, RateLimitStatus::Allowed), "Should allow after window reset");
    }

    #[tokio::test]
    async fn test_cleanup() {
        let limiter = RateLimiter::new();

        // Add some entries
        for i in 0..10 {
            limiter.check_rate_limit("test", Some(&format!("client{}", i))).await.unwrap();
        }

        assert_eq!(limiter.requests.len(), 10);

        // Cleanup should not remove recent entries
        limiter.cleanup_old_entries().await;
        assert_eq!(limiter.requests.len(), 10);
    }
}