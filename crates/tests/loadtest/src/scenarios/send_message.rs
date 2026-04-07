use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

static ITER: AtomicU64 = AtomicU64::new(0);

pub async fn run(
    client: Client,
    base_url: String,
    token: Option<String>,
    metrics: Arc<Metrics>,
    agent_id: &str,
) {
    let iter = ITER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis();

    let auth = match &token {
        Some(t) => format!("Bearer {t}"),
        None => return,
    };

    let body = json!({
        "jsonrpc": "2.0",
        "method": "SendMessage",
        "params": {
            "message": {
                "role": "ROLE_USER",
                "parts": [{"text": format!("Load test message {now}")}],
                "messageId": format!("msg-{now}-{iter}"),
                "contextId": format!("ctx-loadtest-{iter}")
            }
        },
        "id": format!("{now}-{iter}")
    });

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/api/v1/agents/{agent_id}"))
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    sleep(Duration::from_millis(500)).await;
}
