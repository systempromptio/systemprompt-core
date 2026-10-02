//! Router state for the Teams surface: one outbound HTTP client with connect
//! and request timeouts, and one [`ActivityTokenVerifier`] per configured app
//! so each verifier's JWKS cache survives across activities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_identifiers::TeamsAppId;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use systemprompt_models::services::TeamsAppConfig;
use systemprompt_runtime::AppContext;
use systemprompt_teams::auth::ActivityTokenVerifier;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VerifierKey {
    app_id: TeamsAppId,
    openid_config_url: String,
}

#[derive(Clone)]
pub struct TeamsState {
    pub ctx: AppContext,
    pub http: reqwest::Client,
    verifiers: Arc<Mutex<HashMap<VerifierKey, Arc<ActivityTokenVerifier>>>>,
}

impl std::fmt::Debug for TeamsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamsState").finish_non_exhaustive()
    }
}

impl TeamsState {
    pub fn new(ctx: &AppContext) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_DEFAULT_TIMEOUT)
            .build()?;
        Ok(Self {
            ctx: ctx.clone(),
            http,
            verifiers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn verifier(&self, app: &TeamsAppConfig) -> Arc<ActivityTokenVerifier> {
        let key = VerifierKey {
            app_id: TeamsAppId::new(app.app_id.as_str()),
            openid_config_url: app.endpoints.openid_config_url.clone(),
        };
        let mut verifiers = self
            .verifiers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        Arc::clone(verifiers.entry(key).or_insert_with_key(|key| {
            Arc::new(ActivityTokenVerifier::with_openid_url(
                self.http.clone(),
                key.app_id.as_str().to_owned(),
                key.openid_config_url.clone(),
            ))
        }))
    }
}
