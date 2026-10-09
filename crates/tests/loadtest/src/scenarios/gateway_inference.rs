use std::sync::atomic::{AtomicUsize, Ordering};
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

// Set once from the CLI (`--model`, `--session-id`, `--token-file`). A PAT
// carries no session claim, so PAT callers pass the session minted by
// `POST /api/public/gateway/sessions`; `--token-file` holds `token,session_id`
// lines and iterations take them round-robin, one identity per line.
#[derive(Debug, Default)]
pub struct Options {
    pub model: String,
    pub session_id: Option<String>,
    pub credentials: Vec<(String, String)>,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();
static NEXT: AtomicUsize = AtomicUsize::new(0);

pub fn configure(options: Options) {
    let _ = OPTIONS.set(options);
}

pub fn load_token_file(path: &str) -> Result<Vec<(String, String)>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(raw
        .lines()
        .filter_map(|l| l.trim().split_once(','))
        .map(|(t, s)| (t.trim().to_string(), s.trim().to_string()))
        .filter(|(t, s)| !t.is_empty() && !s.is_empty())
        .collect())
}

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let options = OPTIONS.get_or_init(|| Options {
        model: "claude-haiku-4-5".to_string(),
        ..Options::default()
    });
    let (auth, session_id) = if options.credentials.is_empty() {
        let Some(t) = token.as_deref() else {
            return;
        };
        let sid = options.session_id.clone().unwrap_or_else(|| {
            SESSION_ID
                .get_or_init(|| crate::auth::session_id_from_jwt(t).unwrap_or_else(|| t.to_string()))
                .clone()
        });
        (format!("Bearer {t}"), sid)
    } else {
        let i = NEXT.fetch_add(1, Ordering::Relaxed) % options.credentials.len();
        let (t, s) = &options.credentials[i];
        (format!("Bearer {t}"), s.clone())
    };

    let body = json!({
        "model": options.model,
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "ping"}]
    });

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/v1/messages"))
        .header("Authorization", &auth)
        .header("x-session-id", &session_id)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;
    let latency = start.elapsed();
    let success = res.is_ok_and(|r| r.status().is_success());
    metrics.record(latency, success);

    sleep(Duration::from_millis(500)).await;
}
