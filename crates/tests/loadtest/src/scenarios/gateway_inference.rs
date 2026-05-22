use std::sync::{Arc, OnceLock};
use std::time::Instant;

use reqwest::Client;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

// The gateway enforces `x-session-id == token.session_id`, so the header must
// carry the token's own session id (decoded once — it is constant per run),
// not a fresh per-iteration label, or every request 401s before policy eval.
static SESSION_ID: OnceLock<String> = OnceLock::new();

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let Some(t) = token.as_deref() else {
        return;
    };
    let auth = format!("Bearer {t}");
    let session_id = SESSION_ID
        .get_or_init(|| crate::auth::session_id_from_jwt(t).unwrap_or_else(|| t.to_string()));

    let body = json!({
        "model": "claude-haiku-4-5",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "ping"}]
    });

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/v1/messages"))
        .header("Authorization", &auth)
        .header("x-session-id", session_id)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().is_success());
    metrics.record(latency, success);

    sleep(Duration::from_millis(500)).await;
}
