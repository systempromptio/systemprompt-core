//! Auth provider exchanging the device certificate over mTLS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::auth::keystore;
use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::config::Config;
use crate::gateway::GatewayClient;
use crate::gateway::types::{HelperOutput, MtlsRequest};
use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

#[derive(Debug)]
pub struct MtlsProvider {
    base_url: ValidatedUrl,
    cert_ref: Option<String>,
    env_configured: bool,
}

impl MtlsProvider {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let env_configured = std::env::var(crate::brand::brand().env("DEVICE_CERT")).is_ok()
            || std::env::var(crate::brand::brand().env("DEVICE_CERT_LABEL")).is_ok()
            || std::env::var(crate::brand::brand().env("DEVICE_CERT_SHA256")).is_ok();
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            cert_ref: config.cert_keystore_ref().map(|r| r.as_str().to_owned()),
            env_configured,
        }
    }
}

#[async_trait]
impl AuthProvider for MtlsProvider {
    fn name(&self) -> &'static str {
        "mtls"
    }

    async fn authenticate(
        &self,
        session_id: &SessionId,
        http: &reqwest::Client,
    ) -> Result<HelperOutput, AuthError> {
        if !self.env_configured && self.cert_ref.is_none() {
            return Err(AuthError::NotConfigured);
        }

        let cert = keystore::platform_source(self.cert_ref.as_deref())
            .load()
            .map_err(|e| AuthError::Failed {
                provider: "mtls",
                source: AuthFailedSource::Keystore(e),
            })?;

        let req = MtlsRequest {
            device_cert_fingerprint: cert.fingerprint,
        };
        let client = GatewayClient::new(self.base_url.clone(), http.clone());
        let resp = client
            .mtls_exchange(&req, session_id)
            .await
            .map_err(|e| AuthError::Failed {
                provider: "mtls",
                source: AuthFailedSource::Gateway(e),
            })?;
        Ok(resp.into())
    }
}
