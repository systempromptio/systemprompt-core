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
        endpoints: vec!["/v1/models", "/v1/messages"],
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

pub async fn list(headers: HeaderMap) -> Result<Json<ModelsResponse>, (StatusCode, String)> {
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
    let entries = model_entries(&profile.providers, &surfaces);
    let first_id = entries.first().map(|e| e.id.clone());
    let last_id = entries.last().map(|e| e.id.clone());

    Ok(Json(ModelsResponse {
        data: entries,
        has_more: false,
        first_id,
        last_id,
    }))
}

/// Resolve the `x-inference-protocol` selection header into API surfaces. An
/// absent or empty header yields the full catalog (empty slice); an
/// unrecognised tag, or `backend` (never a client surface), is a
/// misconfiguration and fails with `400` rather than silently widening or
/// leaking the advertised set.
fn surfaces_from_header(headers: &HeaderMap) -> Result<Vec<ApiSurface>, (StatusCode, String)> {
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

fn humanize_model_id(id: &str) -> String {
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
