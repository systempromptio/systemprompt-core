//! Request-shape predicates (`when:`), target governance demands
//! (`requires:`) and the profile mirror of the wire response format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_wire::canonical::{CanonicalRequest, ReasoningEffort, ResponseFormat};

use super::token_estimate::estimate_input_tokens;
use crate::services::gateway::error::{GatewayProfileError, GatewayResult};

/// Request-shape predicates a route can require beyond the model glob.
///
/// Every field is optional; an absent predicate is a wildcard, so an empty
/// block matches all requests. The trustworthy discriminators in real agent
/// loops are `thinking` / `min_reasoning_effort` / `stream` and the model name
/// itself — the full tool catalogue is typically resent on every step, so
/// `requires_tools` / `min_tools` are weak signals retained for completeness.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteMatch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_names_any: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tools: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormatKind>,
}

impl RouteMatch {
    #[must_use]
    pub fn matches_request(&self, request: &CanonicalRequest) -> bool {
        (self.tool_names_any.is_empty()
            || request
                .tools
                .iter()
                .any(|tool| self.tool_names_any.contains(&tool.name)))
            && self
                .requires_tools
                .is_none_or(|want| request.tools.is_empty() != want)
            && self.min_tools.is_none_or(|n| request.tools.len() >= n)
            && self
                .thinking
                .is_none_or(|want| request.thinking.is_some_and(|t| t.enabled) == want)
            && self
                .min_reasoning_effort
                .is_none_or(|floor| request.reasoning_effort.is_some_and(|e| e >= floor))
            && self.stream.is_none_or(|want| request.stream == want)
            && self
                .min_input_tokens
                .is_none_or(|n| estimate_input_tokens(request) >= n)
            && self.response_format.is_none_or(|want| {
                ResponseFormatKind::from(request.response_format.as_ref()) == want
            })
    }

    #[must_use]
    pub fn matched_predicates(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.tool_names_any.is_empty() {
            out.push("tool_names_any");
        }
        if self.requires_tools.is_some() {
            out.push("requires_tools");
        }
        if self.min_tools.is_some() {
            out.push("min_tools");
        }
        if self.thinking.is_some() {
            out.push("thinking");
        }
        if self.min_reasoning_effort.is_some() {
            out.push("min_reasoning_effort");
        }
        if self.stream.is_some() {
            out.push("stream");
        }
        if self.min_input_tokens.is_some() {
            out.push("min_input_tokens");
        }
        if self.response_format.is_some() {
            out.push("response_format");
        }
        out
    }

    pub const fn validate(&self) -> GatewayResult<()> {
        if matches!(self.min_tools, Some(0)) {
            return Err(GatewayProfileError::RouteMatchZeroMinTools);
        }
        if let (Some(false), Some(n)) = (self.requires_tools, self.min_tools)
            && n >= 1
        {
            return Err(GatewayProfileError::RouteMatchContradictoryTools);
        }
        Ok(())
    }
}

/// Governance demands a route places on its *target*, not on the request.
///
/// `european: true` restricts the route to providers/models whose effective
/// [`ModelGovernance`](crate::services::ai::ModelGovernance) declares
/// `european`, and `no_retain: true` likewise. Checked at boot for every model
/// the route can reach, and re-checked at dispatch so a selector-refined route
/// or unlisted model cannot bypass it.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteRequirements {
    #[serde(default)]
    pub european: bool,
    #[serde(default)]
    pub no_retain: bool,
}

impl RouteRequirements {
    #[must_use]
    pub fn unmet(&self, governance: crate::services::ai::ModelGovernance) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.european && !governance.european {
            out.push("european");
        }
        if self.no_retain && !governance.no_retain {
            out.push("no_retain");
        }
        out
    }

    #[must_use]
    pub fn declared(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.european {
            out.push("european");
        }
        if self.no_retain {
            out.push("no_retain");
        }
        out
    }
}

/// Profile-side, serializable mirror of the wire [`ResponseFormat`], with an
/// explicit `Text` variant standing in for the wire type's absence (`None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormatKind {
    Text,
    JsonObject,
    JsonSchema,
}

impl From<Option<&ResponseFormat>> for ResponseFormatKind {
    fn from(value: Option<&ResponseFormat>) -> Self {
        match value {
            None => Self::Text,
            Some(ResponseFormat::JsonObject) => Self::JsonObject,
            Some(ResponseFormat::JsonSchema { .. }) => Self::JsonSchema,
        }
    }
}
