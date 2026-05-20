use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::time::{Duration, sleep, timeout};

use crate::metrics::Metrics;

const CONNECTIONS_PER_USER: usize = 4;
const HOLD: Duration = Duration::from_secs(2);

pub async fn run(client: Client, base_url: String, token: Option<String>, metrics: Arc<Metrics>) {
    let mut handles = Vec::with_capacity(CONNECTIONS_PER_USER);

    for _ in 0..CONNECTIONS_PER_USER {
        let client = client.clone();
        let base_url = base_url.clone();
        let token = token.clone();
        let metrics = Arc::clone(&metrics);

        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let mut request = client
                .get(format!("{base_url}/api/v1/core/events/stream"))
                .header("Accept", "text/event-stream");
            if let Some(t) = &token {
                request = request.header("Authorization", format!("Bearer {t}"));
            }

            let response = request.send().await;
            let latency = start.elapsed();

            let success = response.is_ok_and(|r| {
                matches!(r.status().as_u16(), 200 | 401 | 404)
                    && r.headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|v| v.contains("text/event-stream") || r.status().as_u16() != 200)
                        .unwrap_or(true)
            });
            metrics.record(latency, success);
        }));
    }

    for handle in handles {
        if (timeout(HOLD + Duration::from_secs(8), handle).await).is_err() {
            metrics.record(HOLD, false);
        }
    }

    sleep(HOLD).await;
}
