//! Periodic POST `/v1/bridge/heartbeat` from the local bridge to the gateway.
//!
//! Pace is fixed at [`HEARTBEAT_INTERVAL`]. A 401 is reported to the token
//! cache, which decides between renewing and latching for a user sign-in;
//! while latched the tick is answered locally and logs once, not every 30 s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    session_id: &'a systemprompt_identifiers::SessionId,
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
    let mut waiting_for_sign_in = false;
    let mut staging = tokio::task::JoinSet::new();
    loop {
        interval.tick().await;
        while let Some(finished) = staging.try_join_next() {
            if let Err(error) = finished {
                tracing::error!(%error, "required-release staging task did not complete");
            }
        }
        let cfg = runtime_config.load_full();
        flush_feedback(cfg.gateway_base.as_str(), Arc::clone(&session)).await;
        if token_cache.sign_in_required() {
            if !waiting_for_sign_in {
                tracing::warn!("bridge heartbeat paused until the user signs in");
                waiting_for_sign_in = true;
            }
            continue;
        }
        waiting_for_sign_in = false;
        let tick = Tick {
            gateway_base: cfg.gateway_base.as_ref(),
            token_cache: token_cache.as_ref(),
            session: session.as_ref(),
            stats: stats.as_ref(),
            client: &client,
        };
        if let Err(err) = send_one(tick, &mut staging).await {
            tracing::warn!(error = %err, "bridge heartbeat tick failed");
        }
    }
}

async fn flush_feedback(gateway: &str, session: Arc<SessionContext>) {
    let gateway_for_flush = gateway.to_owned();
    let flushed =
        tokio::task::spawn_blocking(move || session.native_sessions().flush(&gateway_for_flush))
            .await
            .map_err(|error| crate::feedback::FeedbackError::Io(std::io::Error::other(error)))
            .and_then(|result| result);
    report_feedback(flushed, "Native session bindings remain unqueued");
    let retried = crate::feedback::retry_pending(gateway).await;
    report_feedback(retried, "Installation feedback retry remains pending");
}

fn report_feedback(result: crate::feedback::Result<()>, message: &'static str) {
    match result {
        Ok(()) | Err(crate::feedback::FeedbackError::EnrollmentRequired) => {},
        Err(error) => tracing::warn!(%error, "{message}"),
    }
}

struct Tick<'a> {
    gateway_base: &'a ValidatedUrl,
    token_cache: &'a TokenCache,
    session: &'a SessionContext,
    stats: &'a ProxyStats,
    client: &'a reqwest::Client,
}

async fn send_one(
    tick: Tick<'_>,
    staging: &mut tokio::task::JoinSet<()>,
) -> Result<(), HeartbeatError> {
    let Tick {
        gateway_base,
        token_cache,
        session,
        stats,
        client,
    } = tick;
    let token = token_cache
        .current(HEARTBEAT_AUTH_THRESHOLD_SECS)
        .await
        .map_err(HeartbeatError::Auth)?;

    let hostname = hostname_or_unknown();
    let payload = HeartbeatPayload {
        session_id: session.session_id(),
        bridge_version: crate::brand::COMPAT_VERSION,
        os: std::env::consts::OS,
        hostname: &hostname,
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
        token_cache.reject_upstream("/v1/bridge/heartbeat").await;
    }
    if !status.is_success() {
        let (status, rejection) = crate::gateway::GatewayRejection::read(response).await;
        return Err(HeartbeatError::Upstream { status, rejection });
    }
    if let Ok(body) = response.json::<HeartbeatResponse>().await
        && !body.compatible
    {
        tracing::warn!(
            local = %crate::brand::COMPAT_VERSION,
            required = %body.min_bridge_version,
            "gateway reports this bridge as unsupported",
        );
        // Why: a staging download can take minutes; the heartbeat keeps its pace.
        staging.spawn(stage_required_release(
            gateway_base.clone(),
            token.token.clone(),
            client.clone(),
        ));
    }
    Ok(())
}

async fn stage_required_release(
    gateway_base: ValidatedUrl,
    bearer: crate::ids::BearerToken,
    http: reqwest::Client,
) {
    use crate::update::automatic::{self, AutomaticError, Outcome, Skipped, Trigger};

    let client = crate::gateway::GatewayClient::new(gateway_base, http);
    match automatic::run(Trigger::Unsupported, &client, &bearer).await {
        Ok(Outcome::Staged { version, .. }) => {
            tracing::info!(%version, "required bridge release staged for the next launch");
        },
        Ok(Outcome::Current)
        | Err(AutomaticError::Skipped(Skipped::NotDue | Skipped::InFlight)) => {},
        Err(AutomaticError::Skipped(skipped @ Skipped::Policy(_))) => {
            tracing::warn!(reason = %skipped, "a newer bridge is required; update manually");
        },
        Err(AutomaticError::Update(e)) => {
            tracing::error!(error = %e, "a newer bridge is required but staging it failed");
        },
    }
}

#[derive(Debug, serde::Deserialize)]
struct HeartbeatResponse {
    min_bridge_version: String,
    compatible: bool,
}

fn i64_saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn hostname_or_unknown() -> String {
    crate::sysproc::host_name().unwrap_or_else(|| "unknown".to_owned())
}

#[derive(Debug, thiserror::Error)]
enum HeartbeatError {
    #[error(transparent)]
    Auth(crate::proxy::forward::ForwardError),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("upstream rejected heartbeat: status {status}: {rejection}")]
    Upstream {
        status: reqwest::StatusCode,
        rejection: Box<crate::gateway::GatewayRejection>,
    },
}
