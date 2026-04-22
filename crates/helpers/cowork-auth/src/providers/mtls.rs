use crate::config::Config;
use crate::http::GatewayClient;
use crate::providers::{AuthError, AuthProvider};
use crate::types::{AuthRequest, HelperOutput};
use crate::{keystore, sso};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MtlsProvider {
    base_url: String,
    configured: bool,
}

impl MtlsProvider {
    pub fn new(config: &Config) -> Self {
        let configured = config
            .mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
            .is_some()
            || std::env::var("SP_COWORK_DEVICE_CERT").is_ok();
        Self {
            base_url: config.gateway_url.clone(),
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
            .map_err(AuthError::Failed)?;
        let assertion = sso::fetch_user_assertion().map_err(AuthError::Failed)?;

        let req = AuthRequest {
            device_cert_fingerprint: cert.fingerprint,
            user_assertion: assertion,
            session_id: new_session_id(),
        };
        let client = GatewayClient::new(self.base_url.clone());
        let resp = client.mtls_exchange(&req).map_err(AuthError::Failed)?;
        Ok(resp.into())
    }
}

fn new_session_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sess-{now:032x}")
}
