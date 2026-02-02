use std::future::Future;
use anyhow::{anyhow, Result};
use tokio::time::{sleep, Duration};
use rand::Rng;

/// Retry configuration
#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            jitter_factor: 0.3,
        }
    }
}

impl RetryConfig {
    /// Calculate delay with exponential backoff and jitter
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.base_delay_ms as f64 * 2.0_f64.powi(attempt as i32);
        let clamped = base.min(self.max_delay_ms as f64);

        // Add jitter: ±jitter_factor of the delay
        let jitter_range = clamped * self.jitter_factor;
        let jitter: f64 = rand::thread_rng().gen_range(-jitter_range..=jitter_range);
        let final_delay = (clamped + jitter).max(0.0);

        Duration::from_millis(final_delay as u64)
    }
}

/// Retry a fallible async operation with exponential backoff
pub async fn retry_async<F, Fut, T>(
    config: &RetryConfig,
    operation_name: &str,
    mut operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < config.max_retries {
                    let delay = config.delay_for_attempt(attempt);
                    eprintln!(
                        "[retry] {} attempt {}/{} failed: {}. Retrying in {:?}",
                        operation_name,
                        attempt + 1,
                        config.max_retries + 1,
                        e,
                        delay
                    );
                    sleep(delay).await;
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("retry_async exhausted without error")))
}

/// Categorize errors for retry decisions
pub fn is_retryable_http_error(status: u16) -> bool {
    matches!(status,
        408 |   // Request Timeout
        429 |   // Too Many Requests
        500 |   // Internal Server Error
        502 |   // Bad Gateway
        503 |   // Service Unavailable
        504     // Gateway Timeout
    )
}

/// Categorize network errors
pub fn is_retryable_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 1000,
            jitter_factor: 0.0, // no jitter for deterministic test
        };

        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
        assert_eq!(config.delay_for_attempt(4), Duration::from_millis(1000)); // clamped
    }

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let config = RetryConfig::default();
        let result: Result<i32> = retry_async(&config, "test", || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_eventual_success() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay_ms: 1, // fast for test
            ..Default::default()
        };

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32> = retry_async(&config, "test", || {
            let c = counter_clone.clone();
            async move {
                let attempt = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if attempt < 2 {
                    Err(anyhow!("not yet"))
                } else {
                    Ok(42)
                }
            }
        }).await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
