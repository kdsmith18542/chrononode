use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;

pub async fn retry_with_backoff<F, Fut, T, E>(
    max_attempts: u32,
    base_delay_ms: u64,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    retry_with_backoff_predicate(max_attempts, base_delay_ms, operation, |_| true).await
}

pub async fn retry_with_backoff_predicate<F, Fut, T, E, P>(
    max_attempts: u32,
    base_delay_ms: u64,
    operation: F,
    predicate: P,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
    P: Fn(&E) -> bool,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        match operation().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                let retryable = predicate(&e);
                if attempt >= max_attempts || !retryable {
                    if !retryable {
                        tracing::debug!("non-retryable error after {} attempts", attempt);
                    } else {
                        tracing::warn!("operation failed after {} attempts: {:?}", attempt, e);
                    }
                    return Err(e);
                }
                let delay = base_delay_ms.saturating_mul(2u64.saturating_pow(attempt - 1));
                let delay = delay.min(60_000);
                tracing::debug!(
                    "attempt {}/{} failed, retrying in {}ms",
                    attempt,
                    max_attempts,
                    delay
                );
                sleep(Duration::from_millis(delay)).await;
            }
        }
    }
}

pub fn retry_with_backoff_fut<F, T, E>(
    max_attempts: u32,
    base_delay_ms: u64,
    operation: F,
) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>> + Send + 'static,
    T: Send + 'static,
    E: std::fmt::Debug + Send + 'static,
{
    Box::pin(retry_with_backoff(max_attempts, base_delay_ms, operation))
}
