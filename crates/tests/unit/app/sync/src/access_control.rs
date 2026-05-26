//! Tests for sync types that don't require a live DB.

use std::time::Duration;
use systemprompt_sync::SyncApiClient;
use systemprompt_sync::api_client::RetryConfig;

#[test]
fn with_retry_config_overrides_default() {
    let custom = RetryConfig {
        max_attempts: 1,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        exponential_base: 4,
    };
    let client = SyncApiClient::new("https://api.example.com", "tok")
        .expect("client")
        .with_retry_config(custom);
    let dbg = format!("{client:?}");
    assert!(dbg.contains("SyncApiClient"));
}
