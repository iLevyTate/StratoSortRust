use crate::error::{AppError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// Connection pool for managing Ollama API connections
pub struct ConnectionPool {
    /// Maximum number of concurrent connections
    max_connections: usize,
    /// Semaphore to limit concurrent requests
    semaphore: Arc<Semaphore>,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
    /// Circuit breaker state
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
}

#[derive(Debug, Clone, Default)]
struct ConnectionStats {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency_ms: u64,
    last_request_time: Option<Instant>,
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    half_open_requests: u32,
    last_failure_time: Option<Instant>,
    consecutive_failures: u32,
    consecutive_successes: u32,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing if service recovered
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            half_open_requests: 0,
            last_failure_time: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
        }
    }
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            semaphore: Arc::new(Semaphore::new(max_connections)),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            circuit_breaker: Arc::new(RwLock::new(CircuitBreaker::default())),
        }
    }

    /// Acquire a connection permit
    pub async fn acquire(&self) -> Result<ConnectionPermit> {
        // Check circuit breaker
        let mut breaker = self.circuit_breaker.write().await;
        breaker.check_state();

        match breaker.state {
            CircuitState::Open => {
                return Err(AppError::AiError {
                    message: "Circuit breaker is open - Ollama service is unavailable".to_string(),
                });
            }
            CircuitState::HalfOpen => {
                if breaker.half_open_requests >= 1 {
                    return Err(AppError::AiError {
                        message: "Circuit breaker is testing - please retry later".to_string(),
                    });
                }
                breaker.half_open_requests += 1;
            }
            CircuitState::Closed => {}
        }

        drop(breaker);

        // Try to acquire permit with timeout
        let permit = match tokio::time::timeout(
            Duration::from_secs(5),
            self.semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(AppError::AiError {
                    message: "Failed to acquire connection permit".to_string(),
                });
            }
            Err(_) => {
                return Err(AppError::Timeout {
                    message: "Connection pool timeout".to_string(),
                });
            }
        };

        let start_time = Instant::now();
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        stats.last_request_time = Some(start_time);
        drop(stats);

        Ok(ConnectionPermit {
            _permit: permit,
            pool: self.clone(),
            start_time,
        })
    }

    /// Record successful request
    pub async fn record_success(&self, latency: Duration) {
        let mut stats = self.stats.write().await;
        stats.successful_requests += 1;
        stats.total_latency_ms += latency.as_millis() as u64;
        drop(stats);

        let mut breaker = self.circuit_breaker.write().await;
        breaker.on_success();
    }

    /// Record failed request
    pub async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.failed_requests += 1;
        drop(stats);

        let mut breaker = self.circuit_breaker.write().await;
        breaker.on_failure();
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionPoolStats {
        let stats = self.stats.read().await;
        let breaker = self.circuit_breaker.read().await;

        let avg_latency_ms = if stats.successful_requests > 0 {
            stats.total_latency_ms / stats.successful_requests
        } else {
            0
        };

        ConnectionPoolStats {
            total_requests: stats.total_requests,
            successful_requests: stats.successful_requests,
            failed_requests: stats.failed_requests,
            avg_latency_ms,
            circuit_breaker_state: format!("{:?}", breaker.state),
            available_connections: self.semaphore.available_permits(),
            max_connections: self.max_connections,
        }
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            max_connections: self.max_connections,
            semaphore: self.semaphore.clone(),
            stats: self.stats.clone(),
            circuit_breaker: self.circuit_breaker.clone(),
        }
    }
}

impl CircuitBreaker {
    /// Check and update circuit breaker state
    fn check_state(&mut self) {
        if self.state == CircuitState::Open {
            if let Some(last_failure) = self.last_failure_time {
                if last_failure.elapsed() >= self.timeout {
                    info!("Circuit breaker transitioning to half-open");
                    self.state = CircuitState::HalfOpen;
                    self.half_open_requests = 0;
                }
            }
        }
    }

    /// Record successful request
    fn on_success(&mut self) {
        self.consecutive_failures = 0;
        self.consecutive_successes += 1;

        if self.state == CircuitState::HalfOpen
            && self.consecutive_successes >= self.success_threshold
        {
            info!("Circuit breaker closing - service recovered");
            self.state = CircuitState::Closed;
            self.half_open_requests = 0;
        }
    }

    /// Record failed request
    fn on_failure(&mut self) {
        self.consecutive_successes = 0;
        self.consecutive_failures += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                if self.consecutive_failures >= self.failure_threshold {
                    error!("Circuit breaker opening - too many failures");
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker reopening - test request failed");
                self.state = CircuitState::Open;
                self.half_open_requests = 0;
            }
            _ => {}
        }
    }
}

/// Permit for making a connection
pub struct ConnectionPermit {
    _permit: tokio::sync::OwnedSemaphorePermit,
    pool: ConnectionPool,
    start_time: Instant,
}

impl ConnectionPermit {
    /// Mark the request as successful
    pub async fn success(self) {
        let latency = self.start_time.elapsed();
        self.pool.record_success(latency).await;
        debug!("Request completed successfully in {:?}", latency);
    }

    /// Mark the request as failed
    pub async fn failure(self) {
        self.pool.record_failure().await;
        debug!("Request failed after {:?}", self.start_time.elapsed());
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionPoolStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: u64,
    pub circuit_breaker_state: String,
    pub available_connections: usize,
    pub max_connections: usize,
}

/// Health check for Ollama server
pub async fn check_ollama_health(host: &str, port: u16) -> Result<bool> {
    use std::net::SocketAddr;
    use tokio::net::TcpStream;

    tracing::debug!("Checking Ollama health at {}:{}", host, port);

    // Try direct connection first with better timeout handling
    let addr = if host == "localhost" {
        format!("127.0.0.1:{}", port)
    } else {
        format!("{}:{}", host, port)
    };

    tracing::debug!("Attempting TCP connection to {}", addr);

    // Parse the socket address directly
    let socket_addr: SocketAddr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            tracing::warn!("Failed to parse address {}: {}", addr, e);
            return Ok(false);
        }
    };

    // Try to connect with async TCP stream and proper timeout
    let connection_result = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(socket_addr)
    ).await;

    match connection_result {
        Ok(Ok(stream)) => {
            drop(stream); // Close the connection immediately
            tracing::debug!("Successfully connected to Ollama at {}", addr);
            
            // Also try a quick HTTP health check to make sure it's actually Ollama
            match check_ollama_http_health(host, port).await {
                Ok(true) => {
                    tracing::debug!("Ollama HTTP health check passed");
                    Ok(true)
                }
                Ok(false) => {
                    tracing::warn!("TCP connection succeeded but HTTP health check failed");
                    Ok(false)
                }
                Err(e) => {
                    tracing::warn!("HTTP health check error: {}", e);
                    // Still return true since TCP connection worked
                    Ok(true)
                }
            }
        }
        Ok(Err(e)) => {
            tracing::warn!("Failed to connect to Ollama at {}: {}", addr, e);
            Ok(false)
        }
        Err(_) => {
            tracing::warn!("Connection timeout when trying to connect to Ollama at {}", addr);
            Ok(false)
        }
    }
}

/// HTTP-based health check for Ollama server
async fn check_ollama_http_health(host: &str, port: u16) -> Result<bool> {
    use reqwest::Client;
    
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| AppError::AiError {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

    let url = if host == "localhost" {
        format!("http://127.0.0.1:{}/", port)
    } else {
        format!("http://{}:{}/", host, port)
    };

    tracing::debug!("Trying HTTP health check at {}", url);

    match client.get(&url).send().await {
        Ok(response) => {
            let is_healthy = response.status().is_success() || response.status() == 404;
            tracing::debug!("HTTP health check response status: {} (healthy: {})", response.status(), is_healthy);
            Ok(is_healthy)
        }
        Err(e) => {
            tracing::warn!("HTTP health check failed: {}", e);
            Ok(false)
        }
    }
}
