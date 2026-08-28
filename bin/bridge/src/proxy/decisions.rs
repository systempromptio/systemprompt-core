//! Periodic GET `/v1/bridge/decisions` joining the gateway's governance
//! verdicts onto the local request ring.
//!
//! The proxy already records what it forwarded and what the gateway answered
//! with; this is what turns that into a governance record, because the verdict
//! itself is only known to the platform. Correlation is by the
//! `x-systemprompt-request-id` the gateway returns on every forwarded response
//! and keys its own audit on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use crate::config::SharedRuntimeConfig;
use crate::gateway::GatewayClient;
use crate::proxy::requests::request_log;
use crate::proxy::token_cache::TokenCache;

pub const DECISIONS_INTERVAL: Duration = Duration::from_secs(30);
const DECISIONS_AUTH_THRESHOLD_SECS: u64 = 300;
const LOOKBACK_SECS: u64 = 15 * 60;

pub async fn run_loop(runtime_config: SharedRuntimeConfig, token_cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(DECISIONS_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        // Why: no forwarded request means nothing to correlate, and polling a
        // decision log for an idle desktop is pure gateway load.
        if request_log().snapshot_recent(1).is_empty() {
            continue;
        }
        let cfg = runtime_config.load_full();
        if let Err(err) = poll_once(cfg.gateway_base.as_ref(), token_cache.as_ref()).await {
            tracing::debug!(error = %err, "governance decision poll failed");
        }
    }
}

async fn poll_once(
    gateway_base: &systemprompt_identifiers::ValidatedUrl,
    token_cache: &TokenCache,
) -> Result<(), String> {
    let token = token_cache
        .current(DECISIONS_AUTH_THRESHOLD_SECS)
        .await
        .map_err(|e| e.to_string())?;
    let since = now_unix().saturating_sub(LOOKBACK_SECS);
    let client = GatewayClient::new(gateway_base.clone());
    let decisions = client
        .fetch_decisions(token.token.expose(), since)
        .await
        .map_err(|e| e.to_string())?;
    let log = request_log();
    for d in decisions.decisions {
        log.apply_gateway_decision(&d.call_id, &d.decision, &d.policy);
    }
    Ok(())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
