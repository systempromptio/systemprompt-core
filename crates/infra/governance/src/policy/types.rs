//! Shared types for the unified governance plane.
//!
//! These types support the governance chain ([`super::GovernancePolicy`]) and
//! feed into the typed deny variants in [`crate::authz::types::DenyReason`].
//! They live here (and not in `authz/types.rs`) because they describe the
//! *governed-call* enforcement plane — secret scans, scope checks, blocklists,
//! rate limits — which is orthogonal to the user→entity allow/deny resolver.
//! What a governed call targets and carries lives in [`super::governed`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CallId, PolicyId, SessionId, UserId};

use super::governed::{GovernedInput, GovernedTarget};
use super::rate_window::RateCounts;
use super::secrets::{SecretFinding, SecretScanner};
use crate::authz::error::AuthzError;
use crate::authz::types::Decision;

/// One governed call before its rate windows are charged.
///
/// [`super::GovernanceEngine::charge`] and
/// [`super::GovernanceEngine::charge_and_evaluate`] turn it into a
/// [`PolicyContext`]; only the engine sets [`PolicyContext::rate_counts`] on
/// the enforcement path.
#[derive(Debug, Clone)]
pub struct PolicyRequest<'a> {
    pub target: GovernedTarget,
    pub agent_scope: AgentScope,
    pub access_scope: AccessScope,
    pub roles: &'a [String],
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub input: &'a GovernedInput,
    pub call_id: &'a CallId,
}

impl<'a> PolicyRequest<'a> {
    pub(crate) fn with_counts<'b>(&'b self, rate_counts: &'b RateCounts) -> PolicyContext<'b>
    where
        'a: 'b,
    {
        PolicyContext {
            target: self.target.clone(),
            agent_scope: self.agent_scope.clone(),
            access_scope: self.access_scope,
            roles: self.roles,
            session_id: self.session_id,
            user_id: self.user_id,
            input: self.input,
            call_id: self.call_id,
            rate_counts,
        }
    }
}
