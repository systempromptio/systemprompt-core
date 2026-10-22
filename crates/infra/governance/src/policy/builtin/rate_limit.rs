//! Per-`{session,user}` fixed-window limiter, registered as the `rate_limit`
//! policy.
//!
//! The count is cluster-wide: the enforcement point charges the window in
//! Postgres before the chain runs (see [`super::super::rate_window`]) and
//! the policy compares the call's position against `requests_per_window`. A
//! call with no charged position — the counter store was unreachable — is a
//! violation, so the limit fails closed under `enforce`.
//!
//! Fixed windows admit up to twice the limit across a window boundary.
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

use serde::Deserialize;
use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;

use super::super::registry::{PolicyConfigurationError, PolicyRegistration};
use super::super::types::{GovernancePolicy, PolicyContext, RateLimitWindow};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

pub(crate) const ID: &str = "rate_limit";
const DEFAULT_WINDOW_SECS: u64 = 60;
const DEFAULT_LIMIT: u64 = 300;

#[derive(Debug)]
struct RateLimit {
    window_secs: u64,
    limit: u64,
}

#[derive(Debug, Default, Deserialize)]
struct RateLimitYaml {
    window_secs: Option<u64>,
    requests_per_window: Option<u64>,
}

impl RateLimit {
    fn from_yaml(v: &YamlValue) -> Result<Self, PolicyConfigurationError> {
        let cfg = serde_yaml::from_value::<Option<RateLimitYaml>>(v.clone())
            .map_err(|source| PolicyConfigurationError::Yaml {
                context: "malformed rate_limit policy entry",
                source,
            })?
            .unwrap_or_default();
        Ok(Self {
            window_secs: cfg.window_secs.unwrap_or(DEFAULT_WINDOW_SECS).max(1),
            limit: cfg.requests_per_window.unwrap_or(DEFAULT_LIMIT),
        })
    }

    fn window(&self) -> RateLimitWindow {
        RateLimitWindow {
            name: ID.to_owned(),
            seconds: self.window_secs,
            limit: self.limit,
        }
    }
}

impl GovernancePolicy for RateLimit {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Rate Limit"
    }
    fn description(&self) -> &'static str {
        "Cluster-wide fixed-window per-session per-user request limiter. Stops a \
         single caller from monopolising the gateway or exfiltrating data via volume."
    }
    fn rate_window(&self) -> Option<RateLimitWindow> {
        Some(self.window())
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let Some(position) = ctx.rate_counts.position(&PolicyId::new(ID)) else {
            return Decision::Deny {
                reason: DenyReason::PolicyViolation {
                    policy: ID.to_owned(),
                    detail: Cow::Borrowed("rate window was not charged; counter unavailable"),
                },
            };
        };

        if position > self.limit {
            Decision::Deny {
                reason: DenyReason::RateLimitExceeded {
                    window: self.window(),
                    retry_after_ms: self.window_secs.saturating_mul(1000),
                },
            }
        } else {
            Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new(ID),
                    detail: Cow::Owned(format!(
                        "{position}/{} calls in {}s window (cluster-wide)",
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
        factory: |v| Ok(Box::new(RateLimit::from_yaml(v)?)),
    }
}
