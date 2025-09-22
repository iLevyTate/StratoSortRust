use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, warn, error};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing if service recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_duration: Duration,
    pub reset_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout_duration: Duration::from_secs(30),
            reset_duration: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for AI service calls
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitBreakerState>>,
    config: CircuitBreakerConfig,
}

#[derive(Debug)]
struct CircuitBreakerState {
    current_state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_success_time: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitBreakerState {
                current_state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                last_success_time: None,
            })),
            config,
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // Check if circuit should allow the call
        if !self.should_allow_request() {
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute with timeout
        let result = timeout(self.config.timeout_duration, operation()).await;

        match result {
            Ok(Ok(value)) => {
                self.on_success();
                Ok(value)
            }
            Ok(Err(e)) => {
                self.on_failure();
                Err(CircuitBreakerError::OperationFailed(e))
            }
            Err(_) => {
                self.on_failure();
                Err(CircuitBreakerError::Timeout)
            }
        }
    }

    /// Check if the circuit should allow a request
    fn should_allow_request(&self) -> bool {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in should_allow_request: {}", e);
                // Default to failing open to avoid complete service unavailability
                return true;
            }
        };

        match state.current_state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if enough time has passed to try half-open
                if let Some(last_failure) = state.last_failure_time {
                    if last_failure.elapsed() >= self.config.reset_duration {
                        debug!("Circuit breaker transitioning to half-open");
                        state.current_state = CircuitState::HalfOpen;
                        state.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Handle successful operation
    fn on_success(&self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in on_success: {}", e);
                return; // Silently fail rather than panic
            }
        };
        state.last_success_time = Some(Instant::now());

        match state.current_state {
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    debug!("Circuit breaker transitioning to closed after {} successes", state.success_count);
                    state.current_state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen, but reset to closed if it does
                warn!("Received success while circuit was open, resetting to closed");
                state.current_state = CircuitState::Closed;
                state.failure_count = 0;
                state.success_count = 0;
            }
        }
    }

    /// Handle failed operation
    fn on_failure(&self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in on_failure: {}", e);
                return; // Silently fail rather than panic
            }
        };
        state.last_failure_time = Some(Instant::now());
        state.failure_count += 1;

        match state.current_state {
            CircuitState::Closed => {
                if state.failure_count >= self.config.failure_threshold {
                    error!("Circuit breaker opening after {} failures", state.failure_count);
                    state.current_state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker returning to open after failure in half-open state");
                state.current_state = CircuitState::Open;
                state.success_count = 0;
            }
            CircuitState::Open => {
                // Already open, just increment failure count
            }
        }
    }

    /// Get current circuit state
    pub fn get_state(&self) -> CircuitState {
        match self.state.lock() {
            Ok(state) => state.current_state,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in get_state: {}", e);
                // Default to Open state to fail fast if mutex is poisoned
                CircuitState::Open
            }
        }
    }

    /// Get failure count
    pub fn get_failure_count(&self) -> u32 {
        match self.state.lock() {
            Ok(state) => state.failure_count,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in get_failure_count: {}", e);
                // Return max failure count to indicate problematic state
                u32::MAX
            }
        }
    }

    /// Reset circuit breaker to closed state
    pub fn reset(&self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(e) => {
                error!("Circuit breaker mutex poisoned in reset: {}", e);
                return; // Silently fail rather than panic
            }
        };
        debug!("Circuit breaker manually reset");
        state.current_state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_failure_time = None;
        state.last_success_time = None;
    }
}

/// Errors that can occur with circuit breaker
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open - failing fast")]
    CircuitOpen,
    #[error("Operation timed out")]
    Timeout,
    #[error("Operation failed: {0:?}")]
    OperationFailed(E),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            reset_duration: Duration::from_millis(500),
        };
        let breaker = CircuitBreaker::new(config);

        // Successful operation should keep circuit closed
        let result = breaker.call(|| async { Ok::<i32, &str>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(breaker.get_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            reset_duration: Duration::from_millis(500),
        };
        let breaker = CircuitBreaker::new(config);

        // First failure
        let result = breaker.call(|| async { Err::<i32, &str>("error") }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        // Second failure should open circuit
        let result = breaker.call(|| async { Err::<i32, &str>("error") }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state(), CircuitState::Open);

        // Third call should fail fast
        let result = breaker.call(|| async { Ok::<i32, &str>(42) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            reset_duration: Duration::from_millis(100),
        };
        let breaker = CircuitBreaker::new(config);

        // Open the circuit
        let _ = breaker.call(|| async { Err::<i32, &str>("error") }).await;
        let _ = breaker.call(|| async { Err::<i32, &str>("error") }).await;
        assert_eq!(breaker.get_state(), CircuitState::Open);

        // Wait for reset duration
        sleep(Duration::from_millis(150)).await;

        // First success should transition to half-open
        let result = breaker.call(|| async { Ok::<i32, &str>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(breaker.get_state(), CircuitState::HalfOpen);

        // Second success should close circuit
        let result = breaker.call(|| async { Ok::<i32, &str>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(breaker.get_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout_duration: Duration::from_millis(50),
            reset_duration: Duration::from_millis(500),
        };
        let breaker = CircuitBreaker::new(config);

        // Operation that takes longer than timeout
        let result = breaker.call(|| async {
            sleep(Duration::from_millis(100)).await;
            Ok::<i32, &str>(42)
        }).await;

        assert!(matches!(result, Err(CircuitBreakerError::Timeout)));
        assert_eq!(breaker.get_failure_count(), 1);
    }
}