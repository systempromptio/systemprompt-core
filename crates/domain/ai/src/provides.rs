//! The `ai` member's two composed provides: the sealed repositories, keyed
//! from the sealed secret store, and the sealed inference provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::{
    DynAiSessionProvider, DynContextMaterializer, SecretLookup, SecretStore, ToolProvider,
};

use crate::repository::{AiRepositories, AiRequestRepositories};
use crate::services::core::{InferenceNotConfigured, InferenceUnavailable};
use crate::services::tools::NoopToolProvider;
use crate::{AiService, AiServiceProviders};

pub(crate) fn repositories(r: &Resolved<'_>) -> Result<Arc<AiRepositories>, ProvideError> {
    let db = r.get::<DbPool>()?;
    let cipher = match r.get::<Arc<dyn SecretStore>>()?.encryption_master_key() {
        Ok(key) => AtRestCipher::from_master_key(key),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "the encryption master key is not held here; personal provider keys refuse every seal and open"
            );
            AtRestCipher::withheld()
        },
    };
    Ok(Arc::new(AiRepositories::new(&db, cipher)))
}

pub(crate) fn inference(r: &Resolved<'_>) -> Result<DynAiProvider, ProvideError> {
    let db = r.get::<DbPool>()?;
    let services = r.get::<ServicesView>()?;
    let tools = r
        .optional::<Arc<dyn ToolProvider>>()
        .unwrap_or_else(|| Arc::new(NoopToolProvider::new()));
    let Some(sessions) = r.optional::<DynAiSessionProvider>() else {
        return Ok(Arc::new(InferenceNotConfigured::new(
            InferenceUnavailable::NoSessionProvider,
        )));
    };
    let providers = AiServiceProviders {
        tools,
        sessions,
        provider_tokens: r.get::<Arc<TokenCache>>()?,
        secrets: r.get::<Arc<dyn SecretLookup>>()?,
        session_ttl_seconds: r.get::<Arc<Config>>()?.jwt_access_token_expiration,
    };
    let config = services.config();
    match AiService::new(
        Arc::clone(&services.catalog),
        &config.ai,
        providers,
        &AiRequestRepositories::new(&db),
    ) {
        Ok(service) => {
            let service = match r.optional::<DynContextMaterializer>() {
                Some(materializer) => service.with_context_materializer(materializer),
                None => service,
            };
            Ok(Arc::new(service))
        },
        Err(error) => {
            tracing::warn!(error = %error, "Inference is not configured; the AI provider refuses every call");
            Ok(Arc::new(InferenceNotConfigured::new(
                InferenceUnavailable::NotConfigured(Arc::new(error)),
            )))
        },
    }
}
