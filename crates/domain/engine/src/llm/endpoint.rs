//! Where and how one call is sent, as the governing layer authorises it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use serde_json::Value;
use url::Url;

/// The target of one model call, supplied by the caller per call.
///
/// `base_url` is the gateway's API root (the protocol path is appended).
/// `headers` carry credentials and are never logged. `metadata`, when it is a
/// JSON object, is merged into the request body at the top level (objects
/// recursively), e.g. `{"metadata": {"user_id": "..."}}`.
#[derive(Clone, PartialEq)]
pub struct Endpoint {
    pub base_url: Url,
    pub headers: Vec<(String, String)>,
    // JSON: caller-owned body fragment merged verbatim into the provider request body.
    pub metadata: Option<Value>,
}

impl Endpoint {
    pub const fn new(base_url: Url) -> Self {
        Self {
            base_url,
            headers: Vec::new(),
            metadata: None,
        }
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Endpoint")
            .field("base_url", &self.base_url.as_str())
            .field("headers", &names)
            .field("metadata", &self.metadata.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}
