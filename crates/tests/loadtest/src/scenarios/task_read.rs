use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let auth = match &token {
        Some(t) => format!("Bearer {t}"),
        None => return,
    };

    let start = Instant::now();
    let res = client
        .get(format!("{base_url}/api/v1/core/tasks"))
        .header("Authorization", &auth)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    sleep(Duration::from_millis(100)).await;
}
