use crate::config::Config;
use crate::providers::{AuthError, AuthProvider};
use crate::types::HelperOutput;

pub struct SessionProvider {
    configured: bool,
}

impl SessionProvider {
    pub fn new(config: &Config) -> Self {
        let configured = config
            .session
            .as_ref()
            .and_then(|s| s.keystore_service.as_ref())
            .is_some();
        Self { configured }
    }
}

impl AuthProvider for SessionProvider {
    fn name(&self) -> &'static str {
        "session"
    }

    fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        if !self.configured {
            return Err(AuthError::NotConfigured);
        }
        Err(AuthError::Failed(
            "session-cookie auth not yet implemented in helper".into(),
        ))
    }
}
