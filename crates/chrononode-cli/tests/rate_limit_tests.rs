use chrononode_cli::api::http::RateLimiter;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_rate_limiter_basic() {
    let limiter = RateLimiter::new(3); // 3 requests per second
    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(!limiter.allow());
}

#[tokio::test]
async fn test_rate_limiter_reset() {
    let limiter = RateLimiter::new(2);
    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(!limiter.allow());

    // Wait for the next second window to start
    sleep(Duration::from_millis(1100)).await;

    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(!limiter.allow());
}

#[tokio::test]
async fn test_rate_limiter_concurrency() {
    let limiter = Arc::new(RateLimiter::new(50));
    let mut handles = vec![];

    for _ in 0..100 {
        let lim = limiter.clone();
        handles.push(tokio::spawn(async move {
            lim.allow()
        }));
    }

    let mut allowed_count = 0;
    for h in handles {
        if h.await.unwrap() {
            allowed_count += 1;
        }
    }

    // Since they all run concurrently within the same second, exactly 50 should be allowed
    assert_eq!(allowed_count, 50);
}
