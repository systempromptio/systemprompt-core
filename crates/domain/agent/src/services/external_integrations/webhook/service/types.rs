use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub secret: Option<String>,
    pub headers: HashMap<String, String>,
    pub timeout: Option<std::time::Duration>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            secret: None,
            headers: HashMap::new(),
            timeout: Some(std::time::Duration::from_secs(30)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebhookDeliveryResult {
    pub success: bool,
    pub status_code: u16,
    pub response_body: String,
    pub response_headers: HashMap<String, String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebhookStats {
    pub endpoint_id: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub last_request_at: Option<chrono::DateTime<chrono::Utc>>,
    pub average_response_time_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WebhookTestResult {
    pub endpoint_id: String,
    pub success: bool,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_factor: 2.0,
        }
    }
}
