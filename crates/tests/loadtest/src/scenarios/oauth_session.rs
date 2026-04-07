use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

pub async fn run(client: Client, base_url: String, _token: Option<String>, metrics: Arc<Metrics>) {
    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/api/v1/core/oauth/session"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    sleep(Duration::from_millis(100)).await;
}
