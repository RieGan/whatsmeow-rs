/// Retry mechanisms with exponential backoff and jitter

use crate::error::{Error, Result};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use serde::{Serialize, Deserialize};

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier (exponential backoff)
    pub backoff_multiplier: f64,
    /// Maximum jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Timeout for each individual attempt
    pub attempt_timeout: Option<Duration>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            attempt_timeout: Some(Duration::from_secs(10)),
        }
    }
}

impl RetryPolicy {
    /// Create a policy for network operations
    pub fn network_operations() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter_factor: 0.2,
            attempt_timeout: Some(Duration::from_secs(30)),
        }
    }
    
    /// Create a policy for critical operations (more aggressive retries)
    pub fn critical_operations() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 1.5,
            jitter_factor: 0.1,
            attempt_timeout: Some(Duration::from_secs(60)),
        }
    }
    
    /// Create a policy for quick operations (fewer retries, shorter delays)
    pub fn quick_operations() -> Self {
        Self {
            max_attempts: 2,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter_factor: 0.05,
            attempt_timeout: Some(Duration::from_secs(5)),
        }
    }
    
    /// Create a policy with no retries
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            initial_delay: Duration::from_secs(0),
            max_delay: Duration::from_secs(0),
            backoff_multiplier: 1.0,
            jitter_factor: 0.0,
            attempt_timeout: None,
        }
    }
    
    /// Calculate delay for given attempt number
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_secs(0);
        }
        
        let base_delay = self.initial_delay.as_secs_f64() 
            * self.backoff_multiplier.powi((attempt - 1) as i32);
        let capped_delay = base_delay.min(self.max_delay.as_secs_f64());
        
        // Add jitter
        let jitter = if self.jitter_factor > 0.0 {
            let jitter_amount = capped_delay * self.jitter_factor;
            fastrand::f64() * jitter_amount
        } else {
            0.0
        };
        
        Duration::from_secs_f64((capped_delay + jitter).min(self.max_delay.as_secs_f64()))
    }
}

/// Information about a retry attempt
#[derive(Debug, Clone)]
pub struct RetryAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Error from previous attempt (None for first attempt)
    pub previous_error: Option<Error>,
    /// Delay before this attempt
    pub delay: Duration,
    /// Total elapsed time since first attempt
    pub elapsed: Duration,
}

/// Result of a retry operation
#[derive(Debug)]
pub enum RetryResult<T> {
    /// Operation succeeded
    Success(T),
    /// Operation failed after all retries
    Failed {
        /// Final error
        error: Error,
        /// All attempts made
        attempts: Vec<RetryAttempt>,
    },
}

/// Retry executor
pub struct RetryExecutor {
    policy: RetryPolicy,
}

impl RetryExecutor {
    /// Create a new retry executor with the given policy
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }
    
    /// Execute an async operation with retries
    pub async fn execute<T, F, Fut>(&self, operation: F) -> RetryResult<T>
    where
        F: Fn(RetryAttempt) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start_time = Instant::now();
        let mut attempts = Vec::new();
        let mut last_error = None;
        
        for attempt_num in 1..=self.policy.max_attempts {
            let delay = if attempt_num == 1 {
                Duration::from_secs(0)
            } else {
                self.policy.calculate_delay(attempt_num - 1)
            };
            
            if delay > Duration::from_secs(0) {
                sleep(delay).await;
            }
            
            let attempt = RetryAttempt {
                attempt: attempt_num,
                previous_error: last_error.clone(),
                delay,
                elapsed: start_time.elapsed(),
            };
            
            attempts.push(attempt.clone());
            
            let result = if let Some(timeout) = self.policy.attempt_timeout {
                match tokio::time::timeout(timeout, operation(attempt)).await {
                    Ok(result) => result,
                    Err(_) => Err(Error::Connection("Operation timeout".to_string())),
                }
            } else {
                operation(attempt).await
            };
            
            match result {
                Ok(value) => return RetryResult::Success(value),
                Err(error) => {
                    if !should_retry(&error) || attempt_num == self.policy.max_attempts {
                        return RetryResult::Failed {
                            error,
                            attempts,
                        };
                    }
                    last_error = Some(error);
                }
            }
        }
        
        // This should never be reached due to the loop structure, but just in case
        RetryResult::Failed {
            error: last_error.unwrap_or_else(|| Error::Connection("Unknown error".to_string())),
            attempts,
        }
    }
    
    /// Execute a sync operation with retries (runs on blocking thread pool)
    pub async fn execute_blocking<T, F>(&self, operation: F) -> RetryResult<T>
    where
        F: Fn(RetryAttempt) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let policy = self.policy.clone();
        
        tokio::task::spawn_blocking(move || {
            let start_time = Instant::now();
            let mut attempts = Vec::new();
            let mut last_error = None;
            
            for attempt_num in 1..=policy.max_attempts {
                let delay = if attempt_num == 1 {
                    Duration::from_secs(0)
                } else {
                    policy.calculate_delay(attempt_num - 1)
                };
                
                if delay > Duration::from_secs(0) {
                    std::thread::sleep(delay);
                }
                
                let attempt = RetryAttempt {
                    attempt: attempt_num,
                    previous_error: last_error.clone(),
                    delay,
                    elapsed: start_time.elapsed(),
                };
                
                attempts.push(attempt.clone());
                
                match operation(attempt) {
                    Ok(value) => return RetryResult::Success(value),
                    Err(error) => {
                        if !should_retry(&error) || attempt_num == policy.max_attempts {
                            return RetryResult::Failed {
                                error,
                                attempts,
                            };
                        }
                        last_error = Some(error);
                    }
                }
            }
            
            RetryResult::Failed {
                error: last_error.unwrap_or_else(|| Error::Connection("Unknown error".to_string())),
                attempts,
            }
        })
        .await
        .unwrap_or_else(|_| RetryResult::Failed {
            error: Error::Connection("Retry task panicked".to_string()),
            attempts: Vec::new(),
        })
    }
}

/// Determine if an error should trigger a retry
pub fn should_retry(error: &Error) -> bool {
    match error {
        // Network errors are generally retryable
        Error::WebSocket(_) => true,
        Error::Connection(_) => true,
        Error::Disconnected(_) => true,
        Error::Io(_) => true,
        
        // Protocol errors usually indicate a permanent issue
        Error::Protocol(_) => false,
        Error::Auth(_) => false,
        Error::Crypto(_) => false,
        Error::InvalidJID(_) => false,
        Error::NotLoggedIn => false,
        
        // Data errors usually indicate a bug
        Error::Json(_) => false,
        Error::ProtobufDecode(_) => false,
        Error::UrlParse(_) => false,
        Error::ElementMissing(_) => false,
        
        // Database errors might be retryable depending on the cause
        Error::Database(msg) => {
            // Simple heuristic: if it mentions connection, it might be retryable
            msg.to_lowercase().contains("connection") || 
            msg.to_lowercase().contains("timeout") ||
            msg.to_lowercase().contains("busy")
        }
        
        // IQ errors depend on the specific error code
        Error::IQ { code, .. } => {
            match code {
                // Temporary errors that might be retryable
                408 => true,       // Request timeout
                429 => true,       // Too many requests
                500..=599 => true, // Server errors (includes 503, 504)
                
                // Client errors that are generally not retryable
                400..=499 => false,
                
                // Other codes
                _ => false,
            }
        }
    }
}

/// Convenience function for retrying an operation
pub async fn retry_operation<T, F, Fut>(
    operation: F,
    policy: RetryPolicy,
) -> Result<T>
where
    F: Fn(RetryAttempt) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let executor = RetryExecutor::new(policy);
    match executor.execute(operation).await {
        RetryResult::Success(value) => Ok(value),
        RetryResult::Failed { error, .. } => Err(error),
    }
}

/// Convenience function for retrying with default network policy
pub async fn retry_network_operation<T, F, Fut>(
    operation: F,
) -> Result<T>
where
    F: Fn(RetryAttempt) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    retry_operation(operation, RetryPolicy::network_operations()).await
}

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open { opened_at: Instant },
    /// Circuit is half-open, testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Time to wait before trying to close circuit
    pub timeout: Duration,
    /// Number of successful requests needed to close circuit from half-open
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
        }
    }
}

/// Simple circuit breaker implementation
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            config,
        }
    }
    
    /// Check if requests are allowed
    pub fn is_request_allowed(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() >= self.config.timeout {
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    /// Record a successful request
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open { .. } => {
                // Shouldn't happen if is_request_allowed is used correctly
            }
        }
    }
    
    /// Record a failed request
    pub fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.state = CircuitState::Open { opened_at: Instant::now() };
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open { opened_at: Instant::now() };
                self.success_count = 0;
            }
            CircuitState::Open { .. } => {
                // Already open, nothing to do
            }
        }
    }
    
    /// Get current state
    pub fn state(&self) -> &CircuitState {
        &self.state
    }
    
    /// Reset circuit breaker to closed state
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[test]
    fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy::default();
        
        // First attempt should have no delay
        assert_eq!(policy.calculate_delay(0), Duration::from_secs(0));
        
        // Subsequent attempts should have exponential backoff
        let delay1 = policy.calculate_delay(1);
        let delay2 = policy.calculate_delay(2);
        let delay3 = policy.calculate_delay(3);
        
        assert!(delay1 < delay2);
        assert!(delay2 < delay3);
        
        // Should respect max delay
        let large_delay = policy.calculate_delay(100);
        assert!(large_delay <= policy.max_delay);
    }
    
    #[tokio::test]
    async fn test_retry_executor_success() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };
        
        let executor = RetryExecutor::new(policy);
        
        let result = executor.execute(|_| async {
            Ok::<i32, Error>(42)
        }).await;
        
        match result {
            RetryResult::Success(value) => assert_eq!(value, 42),
            RetryResult::Failed { .. } => panic!("Should have succeeded"),
        }
    }
    
    #[tokio::test]
    async fn test_retry_executor_eventual_success() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };
        
        let executor = RetryExecutor::new(policy);
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let result = executor.execute(|attempt| {
            let call_count = call_count.clone();
            async move {
                call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if attempt.attempt < 3 {
                    Err(Error::Connection("Temporary failure".to_string()))
                } else {
                    Ok(42)
                }
            }
        }).await;
        
        match result {
            RetryResult::Success(value) => {
                assert_eq!(value, 42);
                assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
            }
            RetryResult::Failed { .. } => panic!("Should have succeeded on third attempt"),
        }
    }
    
    #[tokio::test]
    async fn test_retry_executor_final_failure() {
        let policy = RetryPolicy {
            max_attempts: 2,
            initial_delay: Duration::from_millis(1),
            ..Default::default()
        };
        
        let executor = RetryExecutor::new(policy);
        
        let result = executor.execute(|_| async {
            Err::<i32, Error>(Error::Connection("Persistent failure".to_string()))
        }).await;
        
        match result {
            RetryResult::Success(_) => panic!("Should have failed"),
            RetryResult::Failed { attempts, .. } => {
                assert_eq!(attempts.len(), 2);
            }
        }
    }
    
    #[test]
    fn test_should_retry() {
        // Retryable errors
        assert!(should_retry(&Error::Connection("test".to_string())));
        assert!(should_retry(&Error::WebSocket("connection closed".to_string())));
        assert!(should_retry(&Error::Io("timeout".to_string())));
        
        // Non-retryable errors
        assert!(!should_retry(&Error::Auth("test".to_string())));
        assert!(!should_retry(&Error::Protocol("test".to_string())));
        assert!(!should_retry(&Error::InvalidJID("test".to_string())));
        
        // IQ errors with retryable codes
        assert!(should_retry(&Error::IQ { code: 500, text: "Server error".to_string() }));
        assert!(should_retry(&Error::IQ { code: 429, text: "Rate limited".to_string() }));
        
        // IQ errors with non-retryable codes
        assert!(!should_retry(&Error::IQ { code: 400, text: "Bad request".to_string() }));
        assert!(!should_retry(&Error::IQ { code: 401, text: "Unauthorized".to_string() }));
    }
    
    #[test]
    fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
        };
        
        let mut breaker = CircuitBreaker::new(config);
        
        // Initially closed
        assert_eq!(breaker.state(), &CircuitState::Closed);
        assert!(breaker.is_request_allowed());
        
        // Record failures
        breaker.record_failure();
        assert_eq!(breaker.state(), &CircuitState::Closed);
        
        breaker.record_failure();
        assert!(matches!(breaker.state(), CircuitState::Open { .. }));
        assert!(!breaker.is_request_allowed());
        
        // Should stay closed until timeout
        assert!(!breaker.is_request_allowed());
        
        // Sleep would be needed here to test timeout, but that would slow down tests
        // Instead, we'll test the reset functionality
        breaker.reset();
        assert_eq!(breaker.state(), &CircuitState::Closed);
        assert!(breaker.is_request_allowed());
    }
    
    #[tokio::test]
    async fn test_convenience_functions() {
        let result = retry_operation(
            |_| async { Ok::<i32, Error>(42) },
            RetryPolicy::quick_operations(),
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        let result = retry_network_operation(|_| async {
            Ok::<i32, Error>(123)
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123);
    }
}