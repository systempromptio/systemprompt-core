use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use reqwest::Client;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

static ITER: AtomicU64 = AtomicU64::new(0);

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let iter = ITER.fetch_add(1, Ordering::Relaxed);
    let auth = match &token {
        Some(t) => format!("Bearer {t}"),
        None => return,
    };

    let body = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "agent_id": "loadtest_agent",
        "session_id": format!("loadtest-track-{iter}"),
        "cwd": "/var/www/html/systemprompt-core",
        "tool_input": {"file_path": "/src/main.rs"},
        "tool_result": "file contents"
    });

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/api/public/hooks/track?plugin_id=loadtest"))
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(latency, success);

    sleep(Duration::from_millis(50)).await;
}
