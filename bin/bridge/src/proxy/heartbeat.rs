//! Periodic POST `/v1/bridge/heartbeat` from the local bridge to the gateway.
//! Pace is fixed at [`HEARTBEAT_INTERVAL`]; on auth failure the token cache
//! is invalidated so the next tick re-authenticates.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_identifiers::ValidatedUrl;

use crate::config::SharedRuntimeConfig;
use crate::proxy::server::ProxyStats;
use crate::proxy::session::SessionContext;
use crate::proxy::token_cache::TokenCache;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_AUTH_THRESHOLD_SECS: u64 = 300;

#[derive(Serialize)]
struct HeartbeatPayload<'a> {
    session_id: &'a str,
    bridge_version: &'a str,
    os: &'a str,
    hostname: &'a str,
    last_activity_at: Option<DateTime<Utc>>,
    forwarded_total: i64,
    tokens_in_total: i64,
    tokens_out_total: i64,
}

pub async fn run_loop(
    runtime_config: SharedRuntimeConfig,
    token_cache: Arc<TokenCache>,
    session: Arc<SessionContext>,
    stats: Arc<ProxyStats>,
    client: reqwest::Client,
) {
    let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        let cfg = runtime_config.load_full();
        if let Err(err) = send_one(
            cfg.gateway_base.as_ref(),
            token_cache.as_ref(),
            session.as_ref(),
            stats.as_ref(),
            &client,
        )
        .await
        {
            tracing::warn!(error = %err, "bridge heartbeat tick failed");
        }
    }
}

async fn send_one(
    gateway_base: &ValidatedUrl,
    token_cache: &TokenCache,
    session: &SessionContext,
    stats: &ProxyStats,
    client: &reqwest::Client,
) -> Result<(), HeartbeatError> {
    let token = token_cache
        .current(HEARTBEAT_AUTH_THRESHOLD_SECS)
        .await
        .map_err(|e| HeartbeatError::Auth(e.to_string()))?;

    let payload = HeartbeatPayload {
        session_id: session.session_id().as_str(),
        bridge_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        hostname: hostname_or_unknown(),
        last_activity_at: session.last_activity(),
        forwarded_total: i64_saturating(stats.forwarded_total.load(Ordering::Relaxed)),
        tokens_in_total: i64_saturating(stats.tokens_in_total.load(Ordering::Relaxed)),
        tokens_out_total: i64_saturating(stats.tokens_out_total.load(Ordering::Relaxed)),
    };

    let url = format!(
        "{base}/v1/bridge/heartbeat",
        base = gateway_base.as_str().trim_end_matches('/'),
    );

    let response = client
        .post(&url)
        .bearer_auth(token.token.expose())
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        token_cache.invalidate().await;
    }
    if !status.is_success() {
        return Err(HeartbeatError::Upstream {
            status: status.as_u16(),
        });
    }
    Ok(())
}

fn i64_saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn hostname_or_unknown() -> &'static str {
    static HOSTNAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOSTNAME
        .get_or_init(|| {
            hostname::get()
                .ok()
                .and_then(|os| os.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        })
        .as_str()
}

#[derive(Debug, thiserror::Error)]
enum HeartbeatError {
    #[error("authentication unavailable: {0}")]
    Auth(String),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("upstream rejected heartbeat: status {status}")]
    Upstream { status: u16 },
}
