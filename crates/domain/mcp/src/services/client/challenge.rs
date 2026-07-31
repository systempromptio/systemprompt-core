//! Client-side reading of an MCP server's authorization challenge.
//!
//! Parses the `WWW-Authenticate: Bearer` parameters of a 401 (RFC 6750 §3) into
//! [`AuthChallenge`]. What its `resource_metadata` points at is parsed as the
//! shared [`systemprompt_models::oauth::ProtectedResourceMetadata`], so a
//! caller can tell an Enterprise-Managed Authorization server — one that
//! expects an IdP-issued ID-JAG — apart from one that wants an interactive
//! authorization redirect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Parsed `WWW-Authenticate: Bearer` challenge parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthChallenge {
    pub resource_metadata: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

impl AuthChallenge {
    /// Parse a `WWW-Authenticate` header value.
    ///
    /// Only the `Bearer` scheme is recognised; any other scheme yields an empty
    /// challenge rather than an error, since a server offering something else
    /// is simply one we cannot satisfy.
    #[must_use]
    pub fn parse(header: &str) -> Self {
        let Some(params) = header
            .split_once(char::is_whitespace)
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
            .map(|(_, rest)| rest)
        else {
            return Self::default();
        };

        let mut challenge = Self::default();
        for part in split_params(params) {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let value = unquote(value);
            match key.trim().to_ascii_lowercase().as_str() {
                "resource_metadata" => challenge.resource_metadata = Some(value),
                "error" => challenge.error = Some(value),
                "error_description" => challenge.error_description = Some(value),
                _ => {},
            }
        }
        challenge
    }
}

fn split_params(params: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in params.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(&params[start..idx]);
                start = idx + 1;
            },
            _ => {},
        }
    }
    parts.push(&params[start..]);
    parts
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let Some(inner) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return trimmed.to_owned();
    };

    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;
    for ch in inner.chars() {
        match ch {
            '\\' if !escaped => escaped = true,
            _ => {
                escaped = false;
                out.push(ch);
            },
        }
    }
    out
}

/// Why an MCP server refused a request, as far as the transport can tell.
///
/// The rmcp transport surfaces its client error verbatim, so carrying a typed
/// variant here is what lets a caller branch on "this resource wants an
/// enterprise-managed grant" instead of matching on a message.
#[derive(Debug, thiserror::Error)]
pub enum McpTransportError {
    #[error("{0}")]
    Http(#[from] reqwest::Error),

    #[error(
        "authorization required{}: {reason}",
        if *enterprise_managed { " (enterprise-managed)" } else { "" }
    )]
    AuthorizationRequired {
        reason: String,
        /// Resource identifier from the RFC 9728 metadata, when readable.
        resource: Option<String>,
        /// The `resource_metadata` URL the challenge advertised.
        metadata_url: Option<String>,
        /// Authorization servers the resource delegates to.
        authorization_servers: Vec<String>,
        /// The resource requires an IdP-issued ID-JAG, not a redirect.
        enterprise_managed: bool,
    },
}
