//! The provider call seam: one authorized JSON `POST` to a push provider.
//!
//! [`ProviderHttp`] is the only way a push leaves the process, and
//! [`ProviderCredential`] the only way a push is authorized, so a test drives
//! a sender through stubs and never opens a socket. [`ReqwestProviderHttp`]
//! is the transport a serving instance uses: the workspace client built
//! through the guarded builder, which filters every resolved address and
//! redirect hop. Apple's push service speaks only HTTP/2, which the client
//! negotiates over TLS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_client::{GuardedClientConfig, guarded_client_builder};

use super::PushError;

// Why: the pipeline abandons a channel call at its channel timeout (five
// seconds by default); a provider call outliving it would hold a connection
// for a result nobody reads.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(4);

/// A provider access token, absent from `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderBearer(String);

impl ProviderBearer {
    #[must_use]
    pub const fn new(token: String) -> Self {
        Self(token)
    }

    #[must_use]
    pub const fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for ProviderBearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderBearer(<redacted>)")
    }
}

/// One push as it goes on the wire: the provider URL, the bearer, the
/// provider's own headers and the encoded JSON body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequest {
    pub url: String,
    pub bearer: ProviderBearer,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

/// A provider's answer: the HTTP status and the body text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    pub status: u16,
    pub body: String,
}

/// Sends one request to a push provider; held as `Arc<dyn ProviderHttp>`, so
/// it uses `#[async_trait]`.
#[async_trait]
pub trait ProviderHttp: Send + Sync {
    async fn post(&self, request: ProviderRequest) -> Result<ProviderResponse, PushError>;
}

/// Mints or reuses the bearer a provider call is authorized with; held as
/// `Arc<dyn ProviderCredential>`, so it uses `#[async_trait]`.
#[async_trait]
pub trait ProviderCredential: Send + Sync {
    async fn bearer(&self) -> Result<ProviderBearer, PushError>;
}

/// [`ProviderHttp`] over the workspace HTTP client.
#[derive(Debug, Clone)]
pub struct ReqwestProviderHttp {
    client: reqwest::Client,
}

impl ReqwestProviderHttp {
    pub fn build() -> Result<Self, reqwest::Error> {
        let config = GuardedClientConfig::for_configured().with_timeout(PROVIDER_TIMEOUT);
        Ok(Self {
            client: guarded_client_builder(&config).build()?,
        })
    }
}

#[async_trait]
impl ProviderHttp for ReqwestProviderHttp {
    async fn post(&self, request: ProviderRequest) -> Result<ProviderResponse, PushError> {
        let mut builder = self
            .client
            .post(&request.url)
            .bearer_auth(request.bearer.expose())
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let response = builder
            .body(request.body)
            .send()
            .await
            .map_err(|error| PushError::Transport(Box::new(error)))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| PushError::Transport(Box::new(error)))?;
        Ok(ProviderResponse { status, body })
    }
}
