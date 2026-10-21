//! `/v1/me/provider-credentials` — the caller's personal provider keys.
//!
//! A provider declared `credential: user` is served only from a key the
//! calling user stored for themselves. These routes list which such providers
//! the caller has a key for, store one (after one cheap upstream call proves it
//! works), and remove it. The key is sealed at rest and never returned, logged
//! or echoed in an error; every store and removal is recorded with the user and
//! provider only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::Path;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use systemprompt_ai::UpstreamTarget;
use systemprompt_ai::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};
use systemprompt_loader::ServicesBootstrap;
use systemprompt_manifest::services::ProviderEntry;
use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, SetProviderCredentialRequest,
};
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEY_BYTES: usize = 4096;

pub async fn list(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderCredentialStatus>>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let stored = repository(&ctx)
        .list_metadata(&claims.user_id)
        .await
        .map_err(|e| BridgeError::internal("personal key listing failed", e))?;
    let rows = personal_providers()?
        .iter()
        .map(|entry| {
            let meta = stored.iter().find(|m| m.provider == entry.name.as_str());
            status_row(entry, meta)
        })
        .collect();
    Ok(Json(rows))
}

pub async fn set(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(provider): Path<String>,
    body: Result<Json<SetProviderCredentialRequest>, JsonRejection>,
) -> Result<Json<ProviderCredentialStatus>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;
    let entry = personal_provider(&provider)?;
    let key = body.api_key.trim();
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(BridgeError::PersonalKeyRejected(provider_label(&entry)).into());
    }
    verify_upstream(&entry, key, claims.user_id.as_str()).await?;
    let label = body
        .label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty());
    let meta = repository(&ctx)
        .set_key(&claims.user_id, entry.name.as_str(), key, label)
        .await
        .map_err(|e| BridgeError::internal("personal key could not be stored", e))?;
    tracing::info!(
        user_id = %claims.user_id,
        provider = %entry.name.as_str(),
        "Personal provider key stored"
    );
    Ok(Json(status_row(&entry, Some(&meta))))
}

pub async fn delete(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(provider): Path<String>,
) -> Result<StatusCode, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let entry = personal_provider(&provider)?;
    let removed = repository(&ctx)
        .delete_key(&claims.user_id, entry.name.as_str())
        .await
        .map_err(|e| BridgeError::internal("personal key could not be removed", e))?;
    if removed {
        tracing::info!(
            user_id = %claims.user_id,
            provider = %entry.name.as_str(),
            "Personal provider key removed"
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

fn repository(ctx: &AppContext) -> UserProviderCredentialRepository {
    UserProviderCredentialRepository::new(ctx.db_pool())
}

fn personal_providers() -> Result<Vec<ProviderEntry>, BridgeError> {
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    Ok(services
        .providers
        .providers
        .iter()
        .filter(|entry| entry.is_user_credential())
        .cloned()
        .collect())
}

fn personal_provider(name: &str) -> Result<ProviderEntry, BridgeError> {
    personal_providers()?
        .into_iter()
        .find(|entry| entry.name.as_str() == name)
        .ok_or_else(|| BridgeError::NotPersonalProvider(name.to_owned()))
}

fn provider_label(entry: &ProviderEntry) -> String {
    entry
        .display_name
        .clone()
        .unwrap_or_else(|| entry.name.as_str().to_owned())
}

fn status_row(
    entry: &ProviderEntry,
    meta: Option<&UserProviderCredentialMeta>,
) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: entry.name.as_str().to_owned(),
        display_name: entry.display_name.clone(),
        configured: meta.is_some(),
        label: meta.and_then(|m| m.label.clone()),
        created_at: meta.map(|m| m.created_at),
        verified_at: meta.and_then(|m| m.verified_at),
        last_used_at: meta.and_then(|m| m.last_used_at),
    }
}

// Why: OpenCode Go and every OpenAI- or Anthropic-compatible upstream answer
// `GET {endpoint}/models` for any valid key and 401/403 for a bad one, so it
// proves the key without spending a token of the subscription.
async fn verify_upstream(
    entry: &ProviderEntry,
    key: &str,
    user_id: &str,
) -> Result<(), BridgeError> {
    let unverified = |status: String| BridgeError::PersonalKeyUnverified {
        provider: provider_label(entry),
        status,
    };
    let target = UpstreamTarget::resolve_personal(entry, key, user_id)
        .map_err(|_e| BridgeError::PersonalKeyRejected(provider_label(entry)))?;
    let call = target
        .call()
        .await
        .map_err(|_e| BridgeError::PersonalKeyRejected(provider_label(entry)))?;
    let url = format!("{}/models", call.endpoint().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|e| BridgeError::internal("verification client unavailable", e))?;
    let mut request = client.get(&url);
    for (name, value) in call.headers(entry.wire) {
        if !name.eq_ignore_ascii_case("content-type") {
            request = request.header(name, value);
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_e| unverified("no response".to_owned()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    if matches!(status.as_u16(), 401 | 403) {
        return Err(BridgeError::PersonalKeyRejected(provider_label(entry)));
    }
    Err(unverified(status.as_u16().to_string()))
}
