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
use std::sync::Arc;
use std::{env, fs};
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

pub(crate) fn read_source(config: &Config) -> std::io::Result<Option<PatToken>> {
    let value = match env::var(crate::brand::brand().env("PAT")) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => match config.pat.as_ref().and_then(|p| p.file.as_ref()) {
            Some(path) => Some(
                fs::read_to_string(crate::fsutil::expand_tilde(path)).map_err(|source| {
                    std::io::Error::new(
                        source.kind(),
                        PatReadError {
                            path: path.clone(),
                            source,
                        },
                    )
                })?,
            ),
            None => None,
        },
        Err(e) => return Err(std::io::Error::other(e)),
    };
    value
        .map(|s| {
            if s.trim().is_empty() {
                Err(std::io::Error::other("configured PAT is empty"))
            } else {
                Ok(PatToken::new(s.trim()))
            }
        })
        .transpose()
}

#[derive(Debug, thiserror::Error)]
#[error("read PAT {path}: {source}")]
pub struct PatReadError {
    path: String,
    #[source]
    source: std::io::Error,
}
