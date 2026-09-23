//! How one upstream is addressed: the pair of wire protocol and hosting.
//!
//! The codecs beside this module render a canonical request into a wire body
//! and read the reply back; they are the same wherever the model is hosted.
//! What changes with hosting is the envelope around that body — the URL path,
//! the header an API key travels in, the headers the platform requires, and
//! the few body fields it moves out of the headers or the URL. Claude on
//! Vertex AI is the case that forced this into one place: the Messages body is
//! identical, but it is posted to `:rawPredict`/`:streamRawPredict` on the
//! model's own path, carries `anthropic_version` in the body rather than a
//! header, and names no `model` because the URL already does.
//!
//! [`UpstreamDialect`] is pure: the gateway and the in-process AI service
//! both ask it, so neither can learn a platform the other does not know.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the envelope edits apply to a wire body that is
// dynamic JSON by definition.
use serde_json::{Map, Value};

use std::collections::BTreeSet;

use super::anthropic::{AnthropicBeta, BetaPolicy};
use super::{anthropic, gemini};
use crate::services::providers::{Hosting, WireProtocol};

// Why: Vertex AI's partner-model contract for Claude pins the Messages API
// version in the body under this value; the `anthropic-version` header is
// not read there.
pub const VERTEX_ANTHROPIC_VERSION: &str = "vertex-2023-10-16";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpstreamDialect {
    pub wire: WireProtocol,
    pub hosting: Hosting,
}

impl UpstreamDialect {
    #[must_use]
    pub const fn new(wire: WireProtocol, hosting: Hosting) -> Self {
        Self { wire, hosting }
    }

    #[must_use]
    pub fn of(wire: WireProtocol, endpoint: &str) -> Self {
        Self::new(wire, Hosting::of(endpoint))
    }

    #[must_use]
    pub fn path(self, upstream_model: &str, stream: bool) -> String {
        match (self.wire, self.hosting) {
            (WireProtocol::Anthropic, Hosting::FirstParty) => "/messages".to_owned(),
            (WireProtocol::Anthropic, Hosting::Vertex) => {
                let verb = if stream {
                    "streamRawPredict"
                } else {
                    "rawPredict"
                };
                format!("/models/{upstream_model}:{verb}")
            },
            (WireProtocol::Gemini, _) => gemini::upstream_path(upstream_model, stream),
            (WireProtocol::OpenAiChat, _) => "/chat/completions".to_owned(),
            (WireProtocol::OpenAiResponses, _) => "/responses".to_owned(),
        }
    }

    #[must_use]
    pub fn url(self, endpoint: &str, upstream_model: &str, stream: bool) -> String {
        format!(
            "{}{}",
            endpoint.trim_end_matches('/'),
            self.path(upstream_model, stream)
        )
    }

    #[must_use]
    pub const fn api_key_header(self) -> Option<&'static str> {
        match self.wire {
            WireProtocol::Anthropic => Some("x-api-key"),
            WireProtocol::Gemini => Some(gemini::API_KEY_HEADER),
            WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => None,
        }
    }

    #[must_use]
    pub fn required_headers(self) -> Vec<(&'static str, &'static str)> {
        match (self.wire, self.hosting) {
            (WireProtocol::Anthropic, Hosting::FirstParty) => {
                vec![("anthropic-version", anthropic::ANTHROPIC_VERSION)]
            },
            _ => Vec::new(),
        }
    }

    // Why: Vertex AI rejects an `anthropic-beta` flag it does not serve, and a
    // client forwards the flags it would send Anthropic's own API. With no
    // provider list, Vertex is therefore sent none. The 1M context window is
    // not one of them: Vertex serves Opus 4.6+ and Sonnet 4.6+ at 1,000,000
    // input tokens natively.
    #[must_use]
    pub fn beta_policy(self, accepted: Option<&BTreeSet<AnthropicBeta>>) -> BetaPolicy {
        match (accepted, self.hosting) {
            (Some(accepted), _) => BetaPolicy::Only(accepted.clone()),
            (None, Hosting::FirstParty) => BetaPolicy::ForwardAll,
            (None, Hosting::Vertex) => BetaPolicy::Only(BTreeSet::new()),
        }
    }

    #[must_use]
    pub const fn drops_forwarded_header(self, name: &str) -> bool {
        matches!(
            (self.wire, self.hosting),
            (WireProtocol::Anthropic, Hosting::Vertex)
        ) && name.eq_ignore_ascii_case("anthropic-version")
    }

    pub fn finish_body(self, body: &mut Map<String, Value>) {
        if (self.wire, self.hosting) == (WireProtocol::Anthropic, Hosting::Vertex) {
            body.remove("model");
            body.insert(
                "anthropic_version".to_owned(),
                Value::String(VERTEX_ANTHROPIC_VERSION.to_owned()),
            );
        }
    }

    pub fn finish_value(self, body: &mut Value) {
        if let Some(obj) = body.as_object_mut() {
            self.finish_body(obj);
        }
    }
}
