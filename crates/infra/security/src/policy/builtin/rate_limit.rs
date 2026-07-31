//! `rate_limit`: per-`{session,user}` sliding-window limiter.
//!
//! State is instance-scoped: each engine built by
//! [`super::super::GovernanceEngine::from_config`] gets its own window, so
//! shared enforcement means holding one engine per process.
//!
//! Configurable via:
//! ```yaml
//! - id: rate_limit
//!   requests_per_window: 300
//!   window_secs: 60
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::{CallId, PolicyId, SessionId, UserId};

use super::super::registry::PolicyRegistration;
use super::super::types::{GovernancePolicy, PolicyContext, RateLimitWindow};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

const ID: &str = "rate_limit";
const DEFAULT_WINDOW_SECS: u64 = 60;
const DEFAULT_LIMIT: usize = 300;

#[derive(Debug)]
struct RateLimit {
    window_secs: u64,
    limit: usize,
    counters: Mutex<SlidingWindow>,
}

impl RateLimit {
    fn from_yaml(v: &YamlValue) -> Self {
        let window_secs = v
            .get("window_secs")
            .and_then(YamlValue::as_u64)
            .unwrap_or(DEFAULT_WINDOW_SECS);
        let limit = v
            .get("requests_per_window")
            .and_then(YamlValue::as_u64)
            .map_or(DEFAULT_LIMIT, |n| n as usize);
        Self {
            window_secs,
            limit,
            counters: Mutex::new(SlidingWindow::default()),
        }
    }
}

#[derive(Debug, Default)]
struct SlidingWindow {
    buckets: HashMap<String, Vec<Charge>>,
}

#[derive(Debug)]
struct Charge {
    at: Instant,
    call_id: CallId,
}

const SWEEP_AT: usize = 1024;

impl SlidingWindow {
    fn check_and_record(&mut self, charge: &ChargeRequest<'_>, limit: usize) -> usize {
        let &ChargeRequest {
            key,
            call_id,
            window_secs,
        } = charge;
        let now = Instant::now();
        let cutoff = now
            .checked_sub(Duration::from_secs(window_secs))
            .unwrap_or(now);

        if self.buckets.len() > SWEEP_AT {
            self.buckets.retain(|_, charges| {
                charges.retain(|c| c.at > cutoff);
                !charges.is_empty()
            });
        }

        let charges = self.buckets.entry(key.to_owned()).or_default();
        charges.retain(|c| c.at > cutoff);

        if let Some(position) = charges.iter().position(|c| c.call_id == *call_id) {
            return position;
        }

        let count = charges.len();
        if count < limit {
            charges.push(Charge {
                at: now,
                call_id: call_id.clone(),
            });
        }

        count
    }
}

struct ChargeRequest<'a> {
    key: &'a str,
    call_id: &'a CallId,
    window_secs: u64,
}

fn key_for(session_id: &SessionId, user_id: &UserId) -> String {
    format!("{}:{}", session_id.as_str(), user_id.as_str())
}

impl GovernancePolicy for RateLimit {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Rate Limit"
    }
    fn description(&self) -> &'static str {
        "Sliding-window per-session per-user request limiter. Stops a single \
         caller from monopolising the gateway or exfiltrating data via volume."
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let key = key_for(ctx.session_id, ctx.user_id);
        let count = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .check_and_record(
                &ChargeRequest {
                    key: &key,
                    call_id: ctx.call_id,
                    window_secs: self.window_secs,
                },
                self.limit,
            );

        let window = RateLimitWindow {
            name: ID.to_owned(),
            seconds: self.window_secs,
            limit: self.limit as u64,
        };

        if count >= self.limit {
            Decision::Deny {
                reason: DenyReason::RateLimitExceeded {
                    window,
                    retry_after_ms: self.window_secs.saturating_mul(1000),
                },
            }
        } else {
            Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new(ID),
                    detail: Cow::Owned(format!(
                        "{count}/{} calls in {}s window",
                        self.limit, self.window_secs
                    )),
                },
            }
        }
    }
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Box::new(RateLimit::from_yaml(v)),
    }
}
