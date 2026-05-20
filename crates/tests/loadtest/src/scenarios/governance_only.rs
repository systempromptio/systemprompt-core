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
    let session_id = format!("loadtest-gov-{now}-{iter}");

    let body = json!({
        "model": "denied-model-xyz",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "governance probe"}]
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
    let success = res.is_ok_and(|r| r.status().as_u16() == 403);
    metrics.record(latency, success);

    sleep(Duration::from_millis(100)).await;
}
