//! Auth provider exchanging a stored personal access token for a JWT.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::config::Config;
use crate::gateway::GatewayClient;
use crate::gateway::types::HelperOutput;
use crate::ids::PatToken;
use async_trait::async_trait;
use std::env;
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

#[derive(Debug)]
pub struct PatProvider {
    base_url: ValidatedUrl,
    pat_source: Result<Option<PatToken>, Arc<std::io::Error>>,
}

impl PatProvider {
    pub fn new(config: &Config) -> Self {
        let pat_source = read_source(config).map_err(Arc::new);
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            pat_source,
        }
    }
}

#[async_trait]
impl AuthProvider for PatProvider {
    fn name(&self) -> &'static str {
        "pat"
    }

    async fn authenticate(
        &self,
        session_id: &SessionId,
        http: &reqwest::Client,
    ) -> Result<HelperOutput, AuthError> {
        let pat = self
            .pat_source
            .as_ref()
            .map_err(|e| AuthError::Failed {
                provider: "pat",
                source: AuthFailedSource::Custom(Box::new(Arc::clone(e))),
            })?
            .as_ref()
            .ok_or(AuthError::NotConfigured)?;
        let client = GatewayClient::new(self.base_url.clone(), http.clone());
        let resp = client
            .pat_exchange(pat, session_id)
            .await
            .map_err(|e| AuthError::Failed {
                provider: "pat",
                source: AuthFailedSource::Gateway(e),
            })?;
        Ok(resp.into())
    }
}

#[must_use]
pub fn env_override_present() -> bool {
    env::var(crate::brand::brand().env("PAT"))
        .ok()
        .is_some_and(|s| !s.is_empty())
}

pub(crate) fn read_source(config: &Config) -> std::io::Result<Option<PatToken>> {
    match env::var(crate::brand::brand().env("PAT")) {
        Ok(value) if value.trim().is_empty() => {
            Err(std::io::Error::other("configured PAT is empty"))
        },
        Ok(value) => Ok(Some(PatToken::new(value.trim()))),
        Err(env::VarError::NotPresent) => {
            let Some(store) = config.pat.as_ref().and_then(|p| p.store) else {
                return Ok(None);
            };
            let profile = crate::config::gateways::active()
                .map_err(|e| std::io::Error::other(PatReadError::Profile(e)))?;
            crate::config::gateways::credential::read(&profile, store)
                .map_err(|e| std::io::Error::other(PatReadError::Store(e)))
        },
        Err(e) => Err(std::io::Error::other(e)),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PatReadError {
    #[error("read PAT: {0}")]
    Store(#[source] crate::config::gateways::credential::CredentialError),
    #[error("read PAT: {0}")]
    Profile(#[source] crate::config::gateways::ProfileError),
}
