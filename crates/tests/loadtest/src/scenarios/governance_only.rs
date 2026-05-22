use std::sync::{Arc, OnceLock};
use std::time::Instant;

use reqwest::Client;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::metrics::Metrics;

// The gateway enforces `x-session-id == token.session_id`, so the header must
// carry the token's own session id (decoded once — it is constant per run),
// not a fresh per-iteration label, or every request 401s at auth — before the
// gateway policy can deny the model and we'd never observe the 403 we assert.
static SESSION_ID: OnceLock<String> = OnceLock::new();

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let Some(t) = token.as_deref() else {
        return;
    };
    let auth = format!("Bearer {t}");
    let session_id = SESSION_ID
        .get_or_init(|| crate::auth::session_id_from_jwt(t).unwrap_or_else(|| t.to_string()));

    // A model that is NOT in the gateway allow-list — the deny path. Mirrors the
    // forbidden model used by demo/scenarios/airgap/03-governance.sh, which the
    // gateway rejects with 403 "not permitted by gateway policy".
    let body = json!({
        "model": "claude-opus-forbidden-99",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "governance probe"}]
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
    let success = res.is_ok_and(|r| r.status().as_u16() == 403);
    metrics.record(latency, success);

    sleep(Duration::from_millis(100)).await;
}
