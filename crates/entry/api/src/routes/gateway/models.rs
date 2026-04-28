use axum::Json;
use axum::http::StatusCode;
use serde::Serialize;
use std::collections::BTreeMap;
use systemprompt_models::ProfileBootstrap;

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

pub async fn list() -> Result<Json<ModelsResponse>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let gateway = profile
        .gateway
        .as_ref()
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_string()))?;

    let mut by_id: BTreeMap<String, ModelEntry> = BTreeMap::new();

    if let Some(catalog) = gateway.catalog.as_ref() {
        for m in &catalog.models {
            by_id.insert(
                m.id.clone(),
                ModelEntry {
                    kind: "model",
                    display_name: m
                        .display_name
                        .clone()
                        .unwrap_or_else(|| humanize_model_id(&m.id)),
                    id: m.id.clone(),
                    created_at: "1970-01-01T00:00:00Z".to_string(),
                },
            );
        }
    }

    let entries: Vec<ModelEntry> = by_id.into_values().collect();
    let first_id = entries.first().map(|e| e.id.clone());
    let last_id = entries.last().map(|e| e.id.clone());

    Ok(Json(ModelsResponse {
        data: entries,
        has_more: false,
        first_id,
        last_id,
    }))
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
