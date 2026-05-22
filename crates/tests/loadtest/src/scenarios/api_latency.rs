use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

pub async fn run(client: Client, base_url: String, _token: Option<String>, metrics: Arc<Metrics>) {
    let start = Instant::now();
    let res = client.get(format!("{base_url}/health")).send().await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    // The A2A agent card is served at /.well-known/agent-card.json (the
    // /.well-known/agent.json path 404s on core >= 0.11). Probe the real path
    // and require 200 — a 404 here is a routing regression, not a pass.
    let start = Instant::now();
    let res = client
        .get(format!("{base_url}/.well-known/agent-card.json"))
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    sleep(Duration::from_millis(100)).await;
}
