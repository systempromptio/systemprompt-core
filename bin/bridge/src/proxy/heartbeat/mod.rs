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
use crate::update::automatic::UpdateGate;
use crate::warn_once::WarnOnce;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::instance::{BridgeState, WindowStatus};

mod feedback;
mod release;

use feedback::flush_feedback;
use release::stage_required_release;

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
    release_version: &'a str,
    state: BridgeState,
}

/// What the heartbeat loop runs against, built from the proxy's own state.
pub struct HeartbeatDeps {
    pub runtime_config: SharedRuntimeConfig,
    pub token_cache: Arc<TokenCache>,
    pub session: Arc<SessionContext>,
    pub stats: Arc<ProxyStats>,
    pub client: reqwest::Client,
    pub update_gate: UpdateGate,
}

impl std::fmt::Debug for HeartbeatDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeartbeatDeps").finish_non_exhaustive()
    }
}

pub async fn run_loop(deps: HeartbeatDeps) {
    let HeartbeatDeps {
        runtime_config,
        token_cache,
        session,
        stats,
        client,
        update_gate,
    } = deps;
    let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    let mut waiting_for_sign_in = false;
    let mut failing = false;
    let mut staging = tokio::task::JoinSet::new();
    let feedback_faults = WarnOnce::default();
    loop {
        interval.tick().await;
        while let Some(finished) = staging.try_join_next() {
            if let Err(error) = finished {
                tracing::error!(%error, "required-release staging task did not complete");
            }
        }
        let cfg = runtime_config.load_full();
        flush_feedback(
            cfg.gateway_base.as_str(),
            Arc::clone(&session),
            &feedback_faults,
        )
        .await;
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
            disabled: cfg.disabled,
            token_cache: token_cache.as_ref(),
            session: session.as_ref(),
            stats: stats.as_ref(),
            client: &client,
            update_gate: &update_gate,
        };
        match send_one(tick, &mut staging).await {
            Err(err) if failing => tracing::debug!(error = %err, "bridge heartbeat still failing"),
            Err(err) => {
                tracing::warn!(error = %err, "bridge heartbeat tick failed");
                failing = true;
            },
            Ok(()) if failing => {
                tracing::info!("bridge heartbeat recovered");
                failing = false;
            },
            Ok(()) => {},
        }
    }
}

struct Tick<'a> {
    gateway_base: &'a ValidatedUrl,
    disabled: bool,
    token_cache: &'a TokenCache,
    session: &'a SessionContext,
    stats: &'a ProxyStats,
    client: &'a reqwest::Client,
    update_gate: &'a UpdateGate,
}

async fn send_one(
    tick: Tick<'_>,
    staging: &mut tokio::task::JoinSet<()>,
) -> Result<(), HeartbeatError> {
    let Tick {
        gateway_base,
        disabled,
        token_cache,
        session,
        stats,
        client,
        update_gate,
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
        release_version: crate::brand::brand().version,
        state: if disabled {
            BridgeState::Disabled
        } else {
            BridgeState::Active
        },
    };

    let url = format!(
        "{base}{path}",
        base = gateway_base.as_str().trim_end_matches('/'),
        path = ApiPaths::BRIDGE_HEARTBEAT,
    );

    let response = client
        .post(&url)
        .bearer_auth(token.token.expose())
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        token_cache
            .reject_upstream(ApiPaths::BRIDGE_HEARTBEAT)
            .await;
    }
    if !status.is_success() {
        let (status, rejection) = crate::gateway::GatewayRejection::read(response).await;
        return Err(HeartbeatError::Upstream { status, rejection });
    }
    let Ok(body) = response.json::<HeartbeatResponse>().await else {
        return Ok(());
    };
    let below_window = body.window_status == Some(WindowStatus::Below);
    if !body.compatible {
        tracing::warn!(
            local = %crate::brand::COMPAT_VERSION,
            required = %body.min_bridge_version,
            "gateway reports this bridge as unsupported",
        );
    }
    if below_window {
        tracing::warn!(
            release = %crate::brand::brand().version,
            "gateway reports this bridge release below the window the instance admits",
        );
    }
    if body.window_status == Some(WindowStatus::Above) {
        tracing::warn!(
            release = %crate::brand::brand().version,
            "gateway reports this bridge release above the window the instance admits",
        );
    }
    if !body.compatible || below_window {
        // Why: a staging download can take minutes; the heartbeat keeps its pace.
        staging.spawn(stage_required_release(
            update_gate.clone(),
            gateway_base.clone(),
            token.token.clone(),
            client.clone(),
        ));
    }
    Ok(())
}


#[derive(Debug, serde::Deserialize)]
struct HeartbeatResponse {
    min_bridge_version: String,
    compatible: bool,
    #[serde(default)]
    window_status: Option<WindowStatus>,
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
