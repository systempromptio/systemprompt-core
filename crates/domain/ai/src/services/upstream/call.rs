//! [`UpstreamCall`]: one request's view of a resolved upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the envelope edits apply to a dynamic wire body.
use serde_json::{Map, Value};
use systemprompt_models::services::{Hosting, WireProtocol};
use systemprompt_models::wire::upstream::UpstreamDialect;
use systemprompt_security::credential::{AuthHeader, AuthScheme};

const ANTHROPIC_BETA: &str = "anthropic-beta";

/// The filled endpoint, the minted auth header and the hosting of one upstream,
/// valid for the request it was minted for.
///
/// It carries no wire: the codec that renders the body is the caller's, so
/// every rendering method takes the [`WireProtocol`] and asks the
/// [`UpstreamDialect`] for that pair.
#[derive(Debug, Clone)]
pub struct UpstreamCall {
    hosting: Hosting,
    endpoint: String,
    auth: AuthHeader,
    extra_headers: Vec<(String, String)>,
    accepted_betas: Option<Vec<String>>,
}

impl UpstreamCall {
    #[must_use]
    pub const fn new(
        hosting: Hosting,
        endpoint: String,
        auth: AuthHeader,
        extra_headers: Vec<(String, String)>,
    ) -> Self {
        Self {
            hosting,
            endpoint,
            auth,
            extra_headers,
            accepted_betas: None,
        }
    }

    #[must_use]
    pub fn with_accepted_betas(mut self, accepted_betas: Option<Vec<String>>) -> Self {
        self.accepted_betas = accepted_betas;
        self
    }

    #[must_use]
    pub fn api_key(endpoint: impl Into<String>, key: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self::new(
            Hosting::of(&endpoint),
            endpoint,
            AuthHeader {
                scheme: AuthScheme::ApiKey,
                value: key.into(),
            },
            Vec::new(),
        )
    }

    #[must_use]
    pub fn bearer(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self::new(
            Hosting::of(&endpoint),
            endpoint,
            AuthHeader {
                scheme: AuthScheme::Bearer,
                value: token.into(),
            },
            Vec::new(),
        )
    }

    #[must_use]
    pub const fn hosting(&self) -> Hosting {
        self.hosting
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub const fn is_bearer(&self) -> bool {
        self.auth.is_bearer()
    }

    #[must_use]
    pub const fn dialect(&self, wire: WireProtocol) -> UpstreamDialect {
        UpstreamDialect::new(wire, self.hosting)
    }

    #[must_use]
    pub fn url(&self, wire: WireProtocol, upstream_model: &str, stream: bool) -> String {
        self.dialect(wire)
            .url(&self.endpoint, upstream_model, stream)
    }

    #[must_use]
    pub fn auth_header(&self, wire: WireProtocol) -> (String, String) {
        match (self.auth.scheme, self.dialect(wire).api_key_header()) {
            (AuthScheme::ApiKey, Some(header)) => (header.to_owned(), self.auth.value.clone()),
            _ => (
                "authorization".to_owned(),
                format!("Bearer {}", self.auth.value),
            ),
        }
    }

    #[must_use]
    pub fn headers(&self, wire: WireProtocol) -> Vec<(String, String)> {
        self.headers_forwarding(wire, &[])
    }

    #[must_use]
    pub fn headers_forwarding(
        &self,
        wire: WireProtocol,
        forward: &[(String, String)],
    ) -> Vec<(String, String)> {
        let dialect = self.dialect(wire);
        let mut headers = vec![
            self.auth_header(wire),
            ("content-type".to_owned(), "application/json".to_owned()),
        ];
        headers.extend(
            dialect
                .required_headers()
                .into_iter()
                .filter(|(name, _)| {
                    !forward
                        .iter()
                        .any(|(sent, _)| sent.eq_ignore_ascii_case(name))
                })
                .map(|(name, value)| (name.to_owned(), value.to_owned())),
        );
        headers.extend(
            forward
                .iter()
                .filter(|(name, _)| !dialect.drops_forwarded_header(name))
                .filter_map(|(name, value)| {
                    if wire == WireProtocol::Anthropic && name.eq_ignore_ascii_case(ANTHROPIC_BETA)
                    {
                        self.forwardable_betas(value)
                            .map(|kept| (name.clone(), kept))
                    } else {
                        Some((name.clone(), value.clone()))
                    }
                }),
        );
        headers.extend(self.extra_headers.iter().cloned());
        headers
    }

    fn forwardable_betas(&self, value: &str) -> Option<String> {
        let kept: Vec<&str> = match (&self.accepted_betas, self.hosting) {
            (None, Hosting::FirstParty) => return Some(value.to_owned()),
            (None, Hosting::Vertex) => return None,
            (Some(accepted), _) => value
                .split(',')
                .map(str::trim)
                .filter(|beta| accepted.iter().any(|a| a == beta))
                .collect(),
        };
        (!kept.is_empty()).then(|| kept.join(","))
    }

    pub fn finish_body(&self, wire: WireProtocol, body: &mut Map<String, Value>) {
        self.dialect(wire).finish_body(body);
    }

    pub fn finish_value(&self, wire: WireProtocol, body: &mut Value) {
        self.dialect(wire).finish_value(body);
    }
}
