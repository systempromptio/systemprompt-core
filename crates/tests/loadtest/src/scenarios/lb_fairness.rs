use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

pub async fn run(client: Client, base_url: String, _token: Option<String>, metrics: Arc<Metrics>) {
    let start = Instant::now();
    let res = client.get(format!("{base_url}/health")).send().await;
    let latency = start.elapsed();

    let (success, instance) = match res {
        Ok(r) => {
            let success = r.status().as_u16() == 200;
            let instance = r
                .headers()
                .get("x-served-by")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("unknown")
                .to_string();
            (success, instance)
        },
        Err(_) => (false, "unknown".to_string()),
    };

    metrics.record(latency, success);
    metrics.record_served_by(&instance);

    sleep(Duration::from_millis(100)).await;
}
