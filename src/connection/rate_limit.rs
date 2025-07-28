/// Rate limiting implementation for WhatsApp API calls

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Whether to use sliding window (true) or fixed window (false)
    pub sliding_window: bool,
    /// Burst allowance - extra requests allowed in short bursts
    pub burst_allowance: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 10,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 5,
        }
    }
}

/// Pre-configured rate limits for different WhatsApp operations
pub struct WhatsAppRateLimits;

impl WhatsAppRateLimits {
    /// Rate limit for sending messages
    pub fn message_sending() -> RateLimitConfig {
        RateLimitConfig {
            max_requests: 20,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 5,
        }
    }
    
    /// Rate limit for group operations
    pub fn group_operations() -> RateLimitConfig {
        RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 2,
        }
    }
    
    /// Rate limit for media uploads
    pub fn media_upload() -> RateLimitConfig {
        RateLimitConfig {
            max_requests: 10,
            window_duration: Duration::from_secs(300), // 5 minutes
            sliding_window: true,
            burst_allowance: 3,
        }
    }
    
    /// Rate limit for presence updates
    pub fn presence_updates() -> RateLimitConfig {
        RateLimitConfig {
            max_requests: 30,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 10,
        }
    }
    
    /// Rate limit for contact/status queries
    pub fn contact_queries() -> RateLimitConfig {
        RateLimitConfig {
            max_requests: 50,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 15,
        }
    }
}

/// Rate limiter implementation
pub struct RateLimiter {
    config: RateLimitConfig,
    requests: Arc<Mutex<Vec<Instant>>>,
    burst_tokens: Arc<Mutex<u32>>,
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            burst_tokens: Arc::new(Mutex::new(config.burst_allowance)),
            config,
            requests: Arc::new(Mutex::new(Vec::new())),
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }
    
    /// Check if a request can be made
    pub async fn check_rate_limit(&self) -> RateLimitResult {
        let now = Instant::now();
        
        // Refill burst tokens if needed
        self.refill_burst_tokens(now).await;
        
        // Try to use burst token first
        {
            let mut burst_tokens = self.burst_tokens.lock().await;
            if *burst_tokens > 0 {
                *burst_tokens -= 1;
                return RateLimitResult::Allowed;
            }
        }
        
        // Check normal rate limit
        let mut requests = self.requests.lock().await;
        
        if self.config.sliding_window {
            // Remove requests outside the window
            let window_start = now - self.config.window_duration;
            requests.retain(|&request_time| request_time > window_start);
        } else {
            // Fixed window - clear requests if window has passed
            if let Some(&first_request) = requests.first() {
                if now - first_request >= self.config.window_duration {
                    requests.clear();
                }
            }
        }
        
        // Check if we can make another request
        if requests.len() < self.config.max_requests as usize {
            requests.push(now);
            RateLimitResult::Allowed
        } else {
            // Calculate retry after time
            let retry_after = if self.config.sliding_window {
                // For sliding window, retry after the oldest request expires
                if let Some(&oldest) = requests.first() {
                    let remaining = self.config.window_duration - (now - oldest);
                    remaining
                } else {
                    Duration::from_secs(1)
                }
            } else {
                // For fixed window, retry after window resets
                if let Some(&first_request) = requests.first() {
                    let elapsed = now - first_request;
                    if elapsed < self.config.window_duration {
                        self.config.window_duration - elapsed
                    } else {
                        Duration::from_secs(0)
                    }
                } else {
                    Duration::from_secs(0)
                }
            };
            
            RateLimitResult::Limited { retry_after }
        }
    }
    
    /// Wait for rate limit to allow request
    pub async fn wait_for_rate_limit(&self) -> RateLimitResult {
        loop {
            match self.check_rate_limit().await {
                RateLimitResult::Allowed => return RateLimitResult::Allowed,
                RateLimitResult::Limited { retry_after } => {
                    // Add small jitter to avoid thundering herd
                    let jitter = Duration::from_millis(fastrand::u64(0..=100));
                    tokio::time::sleep(retry_after + jitter).await;
                }
            }
        }
    }
    
    /// Refill burst tokens based on time elapsed
    async fn refill_burst_tokens(&self, now: Instant) {
        let mut last_refill = self.last_refill.lock().await;
        let elapsed = now - *last_refill;
        
        // Refill one burst token per window duration
        let tokens_to_add = (elapsed.as_secs() / self.config.window_duration.as_secs()) as u32;
        
        if tokens_to_add > 0 {
            let mut burst_tokens = self.burst_tokens.lock().await;
            *burst_tokens = (*burst_tokens + tokens_to_add).min(self.config.burst_allowance);
            *last_refill = now;
        }
    }
    
    /// Get current rate limit status
    pub async fn get_status(&self) -> RateLimitStatus {
        let now = Instant::now();
        self.refill_burst_tokens(now).await;
        
        let requests = self.requests.lock().await;
        let burst_tokens = self.burst_tokens.lock().await;
        
        // Count requests in current window
        let window_start = now - self.config.window_duration;
        let current_requests = if self.config.sliding_window {
            requests.iter().filter(|&&req_time| req_time > window_start).count()
        } else {
            if requests.first().map(|&first| now - first < self.config.window_duration).unwrap_or(false) {
                requests.len()
            } else {
                0
            }
        };
        
        RateLimitStatus {
            current_requests: current_requests as u32,
            max_requests: self.config.max_requests,
            window_duration: self.config.window_duration,
            burst_tokens_available: *burst_tokens,
            max_burst_tokens: self.config.burst_allowance,
            time_until_reset: if current_requests > 0 {
                Some(self.config.window_duration - (now - requests[0]))
            } else {
                None
            },
        }
    }
    
    /// Reset rate limiter state
    pub async fn reset(&self) {
        let mut requests = self.requests.lock().await;
        requests.clear();
        
        let mut burst_tokens = self.burst_tokens.lock().await;
        *burst_tokens = self.config.burst_allowance;
        
        let mut last_refill = self.last_refill.lock().await;
        *last_refill = Instant::now();
    }
}

/// Result of rate limit check
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed,
    /// Request is rate limited
    Limited {
        /// Time to wait before retrying
        retry_after: Duration,
    },
}

/// Current rate limit status
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Current number of requests in window
    pub current_requests: u32,
    /// Maximum requests allowed in window
    pub max_requests: u32,
    /// Window duration
    pub window_duration: Duration,
    /// Available burst tokens
    pub burst_tokens_available: u32,
    /// Maximum burst tokens
    pub max_burst_tokens: u32,
    /// Time until window resets
    pub time_until_reset: Option<Duration>,
}

/// Multi-category rate limiter for different operation types
pub struct MultiRateLimiter {
    limiters: HashMap<String, RateLimiter>,
}

impl MultiRateLimiter {
    /// Create a new multi-category rate limiter
    pub fn new() -> Self {
        let mut limiters = HashMap::new();
        
        // Add default WhatsApp rate limiters
        limiters.insert("messages".to_string(), 
                        RateLimiter::new(WhatsAppRateLimits::message_sending()));
        limiters.insert("groups".to_string(), 
                        RateLimiter::new(WhatsAppRateLimits::group_operations()));
        limiters.insert("media".to_string(), 
                        RateLimiter::new(WhatsAppRateLimits::media_upload()));
        limiters.insert("presence".to_string(), 
                        RateLimiter::new(WhatsAppRateLimits::presence_updates()));
        limiters.insert("contacts".to_string(), 
                        RateLimiter::new(WhatsAppRateLimits::contact_queries()));
        
        Self { limiters }
    }
    
    /// Add a custom rate limiter
    pub fn add_limiter(&mut self, category: String, limiter: RateLimiter) {
        self.limiters.insert(category, limiter);
    }
    
    /// Check rate limit for a category
    pub async fn check_rate_limit(&self, category: &str) -> RateLimitResult {
        if let Some(limiter) = self.limiters.get(category) {
            limiter.check_rate_limit().await
        } else {
            RateLimitResult::Allowed
        }
    }
    
    /// Wait for rate limit to allow request in category
    pub async fn wait_for_rate_limit(&self, category: &str) -> RateLimitResult {
        if let Some(limiter) = self.limiters.get(category) {
            limiter.wait_for_rate_limit().await
        } else {
            RateLimitResult::Allowed
        }
    }
    
    /// Get status for all categories
    pub async fn get_all_status(&self) -> HashMap<String, RateLimitStatus> {
        let mut status_map = HashMap::new();
        
        for (category, limiter) in &self.limiters {
            status_map.insert(category.clone(), limiter.get_status().await);
        }
        
        status_map
    }
    
    /// Reset all rate limiters
    pub async fn reset_all(&self) {
        for limiter in self.limiters.values() {
            limiter.reset().await;
        }
    }
}

impl Default for MultiRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let config = RateLimitConfig {
            max_requests: 2,
            window_duration: Duration::from_secs(1),
            sliding_window: true,
            burst_allowance: 1,
        };
        
        let limiter = RateLimiter::new(config);
        
        // First request should be allowed (uses burst token)
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Second request should be allowed (uses regular limit)
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Third request should be allowed (uses regular limit)
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Fourth request should be rate limited
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Limited { .. }));
    }
    
    #[tokio::test]
    async fn test_rate_limiter_window_reset() {
        let config = RateLimitConfig {
            max_requests: 1,
            window_duration: Duration::from_millis(100),
            sliding_window: true,
            burst_allowance: 0,
        };
        
        let limiter = RateLimiter::new(config);
        
        // First request should be allowed
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Second request should be rate limited
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Limited { .. }));
        
        // Wait for window to pass
        sleep(Duration::from_millis(150)).await;
        
        // Now request should be allowed again
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
    }
    
    #[tokio::test]
    async fn test_burst_tokens() {
        let config = RateLimitConfig {
            max_requests: 1,
            window_duration: Duration::from_secs(10),
            sliding_window: true,
            burst_allowance: 2,
        };
        
        let limiter = RateLimiter::new(config);
        
        // Should be able to use burst tokens
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Now should use regular limit
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Allowed));
        
        // Now should be rate limited
        assert!(matches!(limiter.check_rate_limit().await, RateLimitResult::Limited { .. }));
    }
    
    #[tokio::test]
    async fn test_multi_rate_limiter() {
        let multi_limiter = MultiRateLimiter::new();
        
        // Should be able to check different categories
        assert!(matches!(multi_limiter.check_rate_limit("messages").await, RateLimitResult::Allowed));
        assert!(matches!(multi_limiter.check_rate_limit("groups").await, RateLimitResult::Allowed));
        assert!(matches!(multi_limiter.check_rate_limit("media").await, RateLimitResult::Allowed));
        
        // Should handle unknown categories gracefully
        assert!(matches!(multi_limiter.check_rate_limit("unknown").await, RateLimitResult::Allowed));
    }
    
    #[tokio::test]
    async fn test_rate_limit_status() {
        let config = RateLimitConfig {
            max_requests: 5,
            window_duration: Duration::from_secs(60),
            sliding_window: true,
            burst_allowance: 2,
        };
        
        let limiter = RateLimiter::new(config);
        
        let status = limiter.get_status().await;
        assert_eq!(status.current_requests, 0);
        assert_eq!(status.max_requests, 5);
        assert_eq!(status.burst_tokens_available, 2);
        
        // Make a request
        limiter.check_rate_limit().await;
        
        let status = limiter.get_status().await;
        assert_eq!(status.burst_tokens_available, 1);
    }
    
    #[test]
    fn test_whatsapp_rate_limits() {
        // Test that predefined rate limits are reasonable
        let message_limit = WhatsAppRateLimits::message_sending();
        assert!(message_limit.max_requests > 0);
        assert!(message_limit.window_duration > Duration::from_secs(0));
        
        let group_limit = WhatsAppRateLimits::group_operations();
        assert!(group_limit.max_requests > 0);
        assert!(group_limit.max_requests < message_limit.max_requests);
    }
}