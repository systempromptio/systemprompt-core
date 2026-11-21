//! The marketplace route handlers.
//!
//! They serve the default `marketplace.json`, list and render individual
//! marketplaces (JSON and raw `config.yaml`), and stream plugin files. Plugin
//! files are served from `plugin_bundles` — the same scoped, content-gated map
//! the signed manifest is hashed from — so the byte stream and the manifest
//! cannot drift: a plugin the manifest excludes (content-less or in no enabled
//! marketplace) is absent from the map and yields a 404, and an internal
//! `config.yaml` is never part of the generated bundle to leak.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use std::path::Path;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_marketplace::{
    MarketplaceService, render_marketplace_json, render_marketplace_list,
};
use systemprompt_models::api::ApiError;
use systemprompt_models::path_confine::{PathConfinementError, confine_within};

use super::MarketplaceHttpState;
use super::error::{MarketplaceHttpError, MarketplaceRouteError};

pub async fn serve_default_marketplace_json(
    State(state): State<MarketplaceHttpState>,
) -> Result<impl IntoResponse, MarketplaceHttpError> {
    let services = state.services.config();
    let service = MarketplaceService::new(&services);

    let (id, marketplace) = service.resolve_default()?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(id, marketplace))
        .map_err(MarketplaceRouteError::Render)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

pub async fn list_marketplaces(
    State(state): State<MarketplaceHttpState>,
) -> Result<impl IntoResponse, MarketplaceHttpError> {
    let services = state.services.config();
    let service = MarketplaceService::new(&services);

    let body = serde_json::to_vec_pretty(&render_marketplace_list(service.list()))
        .map_err(MarketplaceRouteError::Render)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

pub async fn get_marketplace(
    State(state): State<MarketplaceHttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, MarketplaceHttpError> {
    let id = MarketplaceId::try_new(id).map_err(ApiError::from)?;
    let services = state.services.config();
    let service = MarketplaceService::new(&services);

    let marketplace = service.get(&id)?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(&id, marketplace))
        .map_err(MarketplaceRouteError::Render)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

pub async fn get_marketplace_yaml(
    State(state): State<MarketplaceHttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, MarketplaceHttpError> {
    let id = MarketplaceId::try_new(id).map_err(ApiError::from)?;
    let marketplaces_root = state.services_root.join("marketplaces");
    let requested = marketplaces_root.join(id.as_str()).join("config.yaml");

    let canonical_requested =
        confine_within(&marketplaces_root, &requested).map_err(|error| match error {
            PathConfinementError::Escapes { .. } => {
                ApiHttpError::forbidden("Invalid marketplace id")
            },
            PathConfinementError::RootUnresolved { .. } => {
                ApiHttpError::not_found(format!("Marketplace '{id}' not found"))
            },
            PathConfinementError::Unresolved { .. } | PathConfinementError::NotASegment { .. } => {
                ApiHttpError::not_found(format!("Marketplace '{id}' has no config.yaml"))
            },
        })?;

    if !canonical_requested.is_file() {
        return Err(
            ApiHttpError::not_found(format!("Marketplace '{id}' has no config.yaml")).into(),
        );
    }

    let content = tokio::fs::read(&canonical_requested)
        .await
        .map_err(MarketplaceRouteError::ReadConfig)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/yaml"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        content,
    ))
}

pub async fn serve_plugin_file(
    State(state): State<MarketplaceHttpState>,
    AxumPath((plugin_id, file_path)): AxumPath<(String, String)>,
) -> Result<impl IntoResponse, MarketplaceHttpError> {
    let services = state.services.config();
    let disk_catalog =
        state
            .cache
            .catalog(&services, &state.services_root, &state.api_external_url)?;
    let catalog = (*disk_catalog)
        .clone()
        .without_organization_resources(state.repository.as_ref().clone(), &state.owner)
        .await?;
    let bundles = state.cache.bundles(&services, &catalog.as_content())?;

    let id = PluginId::try_new(&plugin_id)
        .map_err(|_e| ApiHttpError::not_found(format!("Plugin '{plugin_id}' not found")))?;
    let Some(bundle) = bundles.get(&id) else {
        return Err(ApiHttpError::not_found(format!("Plugin '{plugin_id}' not found")).into());
    };

    let Some(file) = bundle.get(&file_path) else {
        return Err(ApiHttpError::not_found(format!("File not found: {file_path}")).into());
    };

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                systemprompt_models::mime::http_content_type(Path::new(&file_path)),
            ),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        file.bytes.clone(),
    ))
}
