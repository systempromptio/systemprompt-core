use crate::auth::keystore;
use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::auth::types::{HelperOutput, MtlsRequest};
use crate::config::Config;
use crate::gateway::GatewayClient;
use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

pub struct MtlsProvider {
    base_url: ValidatedUrl,
    configured: bool,
}

impl MtlsProvider {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let configured = config
            .mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
            .is_some()
            || std::env::var("SP_BRIDGE_DEVICE_CERT").is_ok()
            || std::env::var("SP_BRIDGE_DEVICE_CERT_LABEL").is_ok()
            || std::env::var("SP_BRIDGE_DEVICE_CERT_SHA256").is_ok();
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            configured,
        }
    }
}

#[async_trait]
impl AuthProvider for MtlsProvider {
    fn name(&self) -> &'static str {
        "mtls"
    }

    async fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        if !self.configured {
            return Err(AuthError::NotConfigured);
        }

        let cert = keystore::platform_source()
            .load()
            .map_err(|e| AuthError::Failed {
                provider: "mtls",
                source: AuthFailedSource::Keystore(e),
            })?;

        let req = MtlsRequest {
            device_cert_fingerprint: cert.fingerprint,
            session_id: SessionId::generate(),
        };
        let client = GatewayClient::new(self.base_url.clone());
        let resp = client
            .mtls_exchange(&req)
            .await
            .map_err(|e| AuthError::Failed {
                provider: "mtls",
                source: AuthFailedSource::Gateway(e),
            })?;
        Ok(resp.into())
    }
}
