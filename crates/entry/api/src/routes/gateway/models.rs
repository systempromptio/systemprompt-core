//! `/v1/models` catalog endpoint filtered by inference-protocol surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;
use std::collections::BTreeMap;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::headers::INFERENCE_PROTOCOL;
use systemprompt_models::profile::{ApiSurface, ProviderRegistry};

#[derive(Debug, Serialize)]
pub struct RootResponse {
    pub service: &'static str,
    pub version: &'static str,
    pub endpoints: Vec<&'static str>,
}

pub async fn root() -> Json<RootResponse> {
    Json(RootResponse {
        service: "systemprompt-gateway",
        version: env!("CARGO_PKG_VERSION"),
        endpoints: vec![
            "/v1/models",
            "/v1/messages",
            "/v1/responses",
            "/v1/chat/completions",
        ],
    })
}

#[derive(Debug, Serialize)]
pub struct ModelEntry {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub id: String,
    pub display_name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelEntry>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
}

/// Query parameters accepted by [`list`].
///
/// Gateway model discovery requests `/v1/models?limit=1000`. An unparseable or
/// absent value returns the whole catalog rather than failing: discovery has a
/// three-second budget and treats any non-success as "no models", so a strict
/// parse would cost the developer their picker entries over a cosmetic input.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct ListQuery {
    pub limit: Option<usize>,
    pub format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenAiModelEntry {
    pub id: String,
    pub object: &'static str,
    pub created: u64,
    pub owned_by: &'static str,
}

#[derive(Debug, Serialize)]
pub struct OpenAiModelsResponse {
    pub object: &'static str,
    pub data: Vec<OpenAiModelEntry>,
}

pub async fn list(
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_owned()))?;

    let surfaces = surfaces_from_header(&headers)?;
    let mut entries = model_entries(&profile.providers, &surfaces);
    let total = entries.len();
    let has_more = match query.limit {
        Some(limit) if limit < total => {
            entries.truncate(limit);
            true
        },
        _ => false,
    };

    if query.format.as_deref() == Some("openai") {
        let data = entries
            .into_iter()
            .map(|e| OpenAiModelEntry {
                id: e.id,
                object: "model",
                created: 0,
                owned_by: "systemprompt",
            })
            .collect();
        return Ok(axum::response::IntoResponse::into_response(Json(
            OpenAiModelsResponse {
                object: "list",
                data,
            },
        )));
    }

    let first_id = entries.first().map(|e| e.id.clone());
    let last_id = entries.last().map(|e| e.id.clone());

    Ok(axum::response::IntoResponse::into_response(Json(
        ModelsResponse {
            data: entries,
            has_more,
            first_id,
            last_id,
        },
    )))
}

pub fn surfaces_from_header(headers: &HeaderMap) -> Result<Vec<ApiSurface>, (StatusCode, String)> {
    let Some(raw) = headers
        .get(INFERENCE_PROTOCOL)
        .and_then(|v| v.to_str().ok())
    else {
        return Ok(Vec::new());
    };
    let mut surfaces = Vec::new();
    for tag in raw.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let surface = ApiSurface::from_tag(tag)
            .filter(|s| *s != ApiSurface::Backend)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("unknown {INFERENCE_PROTOCOL} value: {tag}"),
                )
            })?;
        surfaces.push(surface);
    }
    Ok(surfaces)
}

pub fn model_entries(registry: &ProviderRegistry, surfaces: &[ApiSurface]) -> Vec<ModelEntry> {
    let mut by_id: BTreeMap<String, ModelEntry> = BTreeMap::new();
    for id in registry.advertised_model_ids(surfaces) {
        by_id.insert(
            id.clone(),
            ModelEntry {
                kind: "model",
                display_name: humanize_model_id(&id),
                id,
                created_at: "1970-01-01T00:00:00Z".to_owned(),
            },
        );
    }
    by_id.into_values().collect()
}

pub fn humanize_model_id(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |c| {
                c.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}
