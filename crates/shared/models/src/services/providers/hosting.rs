//! Where a provider's models are hosted, as opposed to which wire they speak.
//!
//! A [`WireProtocol`](super::WireProtocol) names the request/response shape;
//! [`Hosting`] names the platform in front of it. The two are independent:
//! Claude speaks the Anthropic wire both on `api.anthropic.com` and on Google
//! Vertex AI, and the two differ in URL shape, auth and a handful of body
//! fields while sharing the codec. Hosting is derived from the endpoint host,
//! never declared, so a catalog entry cannot claim one platform while pointing
//! at another. [`Hosting::of`] is the only place that decision is made.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const VERTEX_HOST: &str = "aiplatform.googleapis.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hosting {
    FirstParty,
    Vertex,
}

impl Hosting {
    #[must_use]
    pub fn of(endpoint: &str) -> Self {
        let on_vertex = url::Url::parse(endpoint)
            .ok()
            .and_then(|url| url.host_str().map(is_vertex_host))
            .unwrap_or(false);
        if on_vertex {
            Self::Vertex
        } else {
            Self::FirstParty
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "first_party",
            Self::Vertex => "vertex",
        }
    }
}

impl std::fmt::Display for Hosting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// Why: Vertex serves from the global host and from regional hosts named
// `<region>-aiplatform.googleapis.com`; both are the same platform.
#[must_use]
pub fn is_vertex_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == VERTEX_HOST || host.ends_with(&format!("-{VERTEX_HOST}"))
}
