//! `require_approval`: hold a matching tool call for a named human.
//!
//! The only policy that returns [`Decision::Pending`]. It does not park or
//! resume anything itself — [`GovernancePolicy::evaluate`] is pure and sync —
//! it only renders the verdict. The enforcement point owns the rendezvous:
//! it writes an `approval_requests` row keyed by [`PolicyContext::call_id`],
//! waits for a decision, and resolves the call.
//!
//! Configurable via:
//! ```yaml
//! - id: require_approval
//!   patterns:
//!   - channel_post                 # every call to a matching tool holds
//!   - tool: email_send             # or narrowed by the call's own arguments
//!     name: external_recipient
//!     when:
//!     - path: to
//!       op: domain_suffix
//!       values: ["systemprompt.io"]
//!       negate: true
//!       match: any
//!   exempt_scopes: ["admin"]
//!   hold_seconds: 60
//!   expiry_seconds: 900
//! ```
//!
//! A bare string behaves exactly as it always has, so an existing config is
//! unchanged. See [`rules`] for the conditional form and its two opposite
//! failure directions.
//!
//! Conditions are safe here only because [`PolicyContext::call_id`] is derived
//! from a digest of the arguments. `evaluate` is contractually idempotent per
//! call id, and a verdict that depends on arguments would break that contract
//! under any id scheme that ignored them — the same id could yield `Pending`
//! one round and `Allow` the next. A retry carrying different arguments derives
//! a different id and addresses a different approval row, so an approval
//! granted on one payload can never be seen by a call carrying another.
//!
//! `patterns` default to **empty**, unlike `tool_blocklist`. The rest of this
//! module's config layer fails toward more enforcement on a bad read, which is
//! right for a policy that refuses; it would be wrong for one that blocks
//! waiting on a human who may not be watching. An unconfigured
//! `require_approval` therefore holds nothing.
//!
//! `hold_seconds` and `expiry_seconds` are read by the enforcement point via
//! [`ApprovalSettings`], not by `evaluate` — they are timings, not verdicts,
//! but they live here so the whole feature is declared in one config block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;

mod operators;
mod rules;

use rules::{Rule, Verdict};

use super::super::config::GovernanceConfig;
use super::super::registry::PolicyRegistration;
use super::super::types::{AccessScope, GovernancePolicy, PolicyContext};
use crate::authz::types::{Decision, MatchedBy, PendingReason};

pub(crate) const ID: &str = "require_approval";

const DEFAULT_HOLD_SECONDS: u64 = 60;

const DEFAULT_EXPIRY_SECONDS: u64 = 900;

/// The timing half of the `require_approval` config, for the enforcement point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalSettings {
    pub hold_seconds: u64,
    pub expiry_seconds: u64,
}

impl Default for ApprovalSettings {
    fn default() -> Self {
        Self {
            hold_seconds: DEFAULT_HOLD_SECONDS,
            expiry_seconds: DEFAULT_EXPIRY_SECONDS,
        }
    }
}

impl ApprovalSettings {
    #[must_use]
    pub fn from_governance_config(config: &GovernanceConfig) -> Self {
        config
            .policies
            .iter()
            .find(|p| p.id == ID)
            .map_or_else(Self::default, |p| Self::from_params(&p.params))
    }

    #[must_use]
    pub fn from_params(v: &YamlValue) -> Self {
        let default = Self::default();
        Self {
            hold_seconds: positive_u64(v, "hold_seconds").unwrap_or(default.hold_seconds),
            expiry_seconds: positive_u64(v, "expiry_seconds").unwrap_or(default.expiry_seconds),
        }
    }
}

// Why: a zero here would mean "hold for no time at all" / "expire instantly",
// which is a config typo rather than an intent worth honouring.
fn positive_u64(v: &YamlValue, key: &str) -> Option<u64> {
    v.get(key).and_then(YamlValue::as_u64).filter(|n| *n > 0)
}

#[derive(Debug)]
struct RequireApproval {
    rules: Vec<Rule>,
    exempt_scopes: Vec<AccessScope>,
}

impl RequireApproval {
    fn from_yaml(v: &YamlValue) -> Self {
        let exempt_scopes = string_list(v, "exempt_scopes")
            .iter()
            .filter_map(|s| parse_scope(s))
            .collect();
        Self {
            rules: rules::compile(v),
            exempt_scopes,
        }
    }
}

fn string_list(v: &YamlValue, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(YamlValue::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|p| p.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_scope(raw: &str) -> Option<AccessScope> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "admin" => Some(AccessScope::Admin),
        "user" => Some(AccessScope::User),
        "unknown" => Some(AccessScope::Unknown),
        other => {
            tracing::warn!(
                scope = other,
                policy = ID,
                "unknown access scope in exempt_scopes — ignoring"
            );
            None
        },
    }
}

impl GovernancePolicy for RequireApproval {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Require Approval"
    }
    fn description(&self) -> &'static str {
        "Hold matching tool calls for an explicit human approval before they run, \
         instead of allowing or denying them outright."
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let allow = |detail| Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new(ID),
                detail,
            },
        };
        let Some(tool) = ctx.target.tool() else {
            return allow(Cow::Borrowed("Not a tool call"));
        };
        if self.exempt_scopes.contains(&ctx.access_scope) {
            return allow(Cow::Borrowed("Caller scope is exempt from approval"));
        }
        // Why: computed at most once per call, and only once a tool name has
        // actually matched — the overwhelming majority of calls match nothing
        // and must not pay for a walk of their own arguments.
        let mut scalars = None;
        for rule in &self.rules {
            if !rule.matches_tool(tool.as_str()) {
                continue;
            }
            let scalars = scalars.get_or_insert_with(|| ctx.input.scalars());
            if let Verdict::Hold(rule) = rule.evaluate(scalars) {
                return Decision::Pending {
                    reason: PendingReason::ApprovalRequired {
                        tool: tool.clone(),
                        rule,
                    },
                };
            }
        }
        allow(Cow::Borrowed("Tool does not require approval"))
    }
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Box::new(RequireApproval::from_yaml(v)),
    }
}
