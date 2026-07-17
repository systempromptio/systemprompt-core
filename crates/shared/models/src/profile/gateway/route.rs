//! Gateway routing patterns and stable route-id synthesis.
//!
//! A [`GatewayRoute`] maps an external `model_pattern` (exact, prefix `foo*`,
//! suffix `*foo`, or catch-all `*`) onto a provider in the registry. When a
//! route omits an explicit id, [`synthesize_route_id`] derives a stable one
//! from `(model_pattern, provider)` so `access_control_rules` can address the
//! route by a name that survives reordering. A model's connectivity is never
//! embedded here — [`GatewayRoute::resolve`] looks the provider up in the
//! registry at use time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::super::providers::{ProviderEntry, ProviderRegistry};
use super::error::{GatewayProfileError, GatewayResult};
use crate::gateway_hash::fnv1a_segments;
use crate::services::ai::ModelPricing;
use crate::wire::canonical::{CanonicalContent, CanonicalRequest, ReasoningEffort, ResponseFormat};

fn default_route_id() -> RouteId {
    RouteId::new("")
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayRoute {
    #[serde(default = "default_route_id")]
    pub id: RouteId,
    pub model_pattern: String,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
    /// Optional request-shape predicates. Absent (`None`) preserves the
    /// model-only matching behaviour; present narrows the route to requests
    /// whose attributes satisfy every set predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<RouteMatch>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(&self.model_pattern, model)
    }

    /// Model-glob match plus, when a [`RouteMatch`] is present, every set
    /// request-shape predicate. A route without a `when` block matches purely
    /// on model name, exactly as [`Self::matches`].
    pub fn matches_request(&self, request: &CanonicalRequest) -> bool {
        self.matches(&request.model)
            && self
                .when
                .as_ref()
                .is_none_or(|w| w.matches_request(request))
    }

    /// Returns the requested name verbatim when the route carries no
    /// `upstream_model` rewrite. For the synthesized `*` default route this
    /// passthrough is intentional — see
    /// `GatewayConfig::synthesize_default_route`.
    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }

    pub fn ensure_id(&mut self) {
        if self.id.as_str().trim().is_empty() {
            self.id = synthesize_route_id(&self.model_pattern, self.provider.as_str());
        }
    }

    pub fn resolve<'a>(&self, registry: &'a ProviderRegistry) -> Option<&'a ProviderEntry> {
        registry.find_provider(self.provider.as_str())
    }
}

/// Request-shape predicates a route can require beyond the model glob.
///
/// Every field is optional; an absent predicate is a wildcard, so an empty
/// block matches all requests. The trustworthy discriminators in real agent
/// loops are `thinking` / `min_reasoning_effort` / `stream` and the model name
/// itself — the full tool catalogue is typically resent on every step, so
/// `requires_tools` / `min_tools` are weak signals retained for completeness.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteMatch {
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
    /// True when every set predicate holds for `request`; absent predicates are
    /// ignored.
    #[must_use]
    pub fn matches_request(&self, request: &CanonicalRequest) -> bool {
        self.requires_tools
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

    /// Names of the predicates that are set, in declaration order — the basis
    /// for the route-match audit descriptor.
    #[must_use]
    pub fn matched_predicates(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
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

    /// Rejects predicate combinations that can never match: a zero `min_tools`
    /// (vacuous), and `requires_tools: false` paired with a positive
    /// `min_tools` (contradiction).
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

/// Deliberately approximate input-token estimate (`chars / 4 + 1`) over the
/// system prompt and message text — cheap and dependency-free. Precise
/// tokenisation, when needed, belongs in a [`super::super`]-side
/// `RouteSelector`.
fn estimate_input_tokens(request: &CanonicalRequest) -> u32 {
    let mut chars = request.system.as_deref().map_or(0, str::len);
    for message in &request.messages {
        for part in &message.content {
            accumulate_text_len(part, &mut chars);
        }
    }
    u32::try_from(chars / 4 + 1).unwrap_or(u32::MAX)
}

fn accumulate_text_len(part: &CanonicalContent, acc: &mut usize) {
    match part {
        CanonicalContent::Text(t) => *acc += t.len(),
        CanonicalContent::Thinking { text, .. } => *acc += text.len(),
        CanonicalContent::ToolResult { content, .. } => {
            for inner in content {
                accumulate_text_len(inner, acc);
            }
        },
        CanonicalContent::ToolUse { .. } | CanonicalContent::Image(_) => {},
    }
}

#[must_use]
pub fn slugify_pattern(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut last_dash = false;
    for ch in pattern.chars() {
        if ch == '*' {
            out.push_str("star");
            last_dash = false;
        } else if ch.is_ascii_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    if out.is_empty() {
        out.push_str("route");
    }
    out
}

#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str) -> RouteId {
    let h = fnv1a_segments(&[
        ("model_pattern", model_pattern.as_bytes()),
        ("provider", provider.as_bytes()),
    ]);
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    RouteId::new(format!("{}-{}", slugify_pattern(model_pattern), hash6))
}

pub(crate) fn match_pattern(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}
