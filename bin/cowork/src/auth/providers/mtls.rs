use crate::auth::keystore;
use crate::auth::providers::{AuthError, AuthProvider};
use crate::auth::types::{HelperOutput, MtlsRequest};
use crate::config::Config;
use crate::gateway::GatewayClient;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

pub struct MtlsProvider {
    base_url: ValidatedUrl,
    configured: bool,
}

impl MtlsProvider {
    pub fn new(config: &Config) -> Self {
        let configured = config
            .mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
            .is_some()
            || std::env::var("SP_COWORK_DEVICE_CERT").is_ok()
            || std::env::var("SP_COWORK_DEVICE_CERT_LABEL").is_ok()
            || std::env::var("SP_COWORK_DEVICE_CERT_SHA256").is_ok();
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            configured,
        }
    }
}

impl AuthProvider for MtlsProvider {
    fn name(&self) -> &'static str {
        "mtls"
    }

    fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        if !self.configured {
            return Err(AuthError::NotConfigured);
        }

        let cert = keystore::platform_source()
            .load()
            .map_err(|e| AuthError::Failed(e.to_string()))?;

        let req = MtlsRequest {
            device_cert_fingerprint: cert.fingerprint,
            session_id: SessionId::generate(),
        };
        let client = GatewayClient::new(self.base_url.clone());
        let resp = client
            .mtls_exchange(&req)
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(resp.into())
    }
}
