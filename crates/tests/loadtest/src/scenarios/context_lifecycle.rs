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

    let start = Instant::now();
    let res = client
        .post(format!("{base_url}/api/v1/core/contexts"))
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .json(&json!({"name": format!("loadtest-{iter}")}))
        .send()
        .await;
    let create_latency = start.elapsed();

    let context_id = match res {
        Ok(r) if r.status().as_u16() == 200 || r.status().as_u16() == 201 => {
            let body: serde_json::Value = match r.json().await {
                Ok(v) => v,
                Err(_) => {
                    metrics.record(create_latency, false);
                    return;
                }
            };
            match body["data"]["context_id"].as_str() {
                Some(id) => id.to_string(),
                None => {
                    metrics.record(create_latency, false);
                    return;
                }
            }
        }
        _ => {
            metrics.record(create_latency, false);
            return;
        }
    };
    metrics.record(create_latency, true);

    let start = Instant::now();
    let res = client
        .get(format!("{base_url}/api/v1/core/contexts/{context_id}"))
        .header("Authorization", &auth)
        .send()
        .await;
    let read_latency = start.elapsed();
    let read_ok = res.is_ok_and(|r| r.status().as_u16() == 200);
    metrics.record(read_latency, read_ok);

    let cleanup = client
        .delete(format!("{base_url}/api/v1/core/contexts/{context_id}"))
        .header("Authorization", &auth)
        .send()
        .await;
    if let Err(e) = cleanup {
        eprintln!("    cleanup failed for context {context_id}: {e}");
    }

    sleep(Duration::from_millis(200)).await;
}
