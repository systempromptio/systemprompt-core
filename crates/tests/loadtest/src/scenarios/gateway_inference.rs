use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

static ITER: AtomicU64 = AtomicU64::new(0);

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let auth = match &token {
        Some(t) => format!("Bearer {t}"),
        None => return,
    };

    let iter = ITER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis();
    let session_id = format!("loadtest-infer-{now}-{iter}");

    let body = json!({
        "model": "claude-haiku-4-5",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "ping"}]
    });

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/v1/messages"))
        .header("Authorization", &auth)
        .header("x-session-id", &session_id)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().is_success());
    metrics.record(latency, success);

    sleep(Duration::from_millis(500)).await;
}
