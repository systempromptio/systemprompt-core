//! Managed sources, revision bundles and Git dependency verification,
//! independent of any UI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::ManagedHttpError;
use super::state::ManagedState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_identifiers::{ManagedSourceId, ResourceRevisionId};
use systemprompt_marketplace::managed::{GitSourceBinding, RevisionBundle, SourceSpec};
use systemprompt_models::feedback::verification::DependencyVerificationManifest;
use systemprompt_runtime::AppContext;

pub(super) fn router() -> Router<ManagedState> {
    Router::new()
        .route("/sources", post(create_source))
        .route("/sources/{id}", get(source))
        .route(
            "/sources/{id}/verification-bindings",
            post(bind_verification_source),
        )
        .route(
            "/sources/{id}/captures",
            post(super::operation_handlers::capture),
        )
        .route("/revisions/{id}/bundle", get(bundle))
        .route(
            "/source-verifications",
            post(super::operation_handlers::verify),
        )
        .route("/source-verifications/{id}", get(source_verification))
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateSource {
    name: String,
    specification: SourceSpec,
}

async fn create_source(
    State(ctx): State<AppContext>,
    Json(input): Json<CreateSource>,
) -> Result<impl axum::response::IntoResponse, ManagedHttpError> {
    let id = ctx
        .managed_repository()
        .register_source(ctx.system_admin().id(), &input.name, &input.specification)
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/sources/{id}"))],
        Json(id),
    ))
}

async fn source(
    State(ctx): State<AppContext>,
    Path(id): Path<ManagedSourceId>,
) -> Result<Json<SourceSpec>, ManagedHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .get_source(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn bundle(
    State(ctx): State<AppContext>,
    Path(id): Path<ResourceRevisionId>,
) -> Result<Json<RevisionBundle>, ManagedHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .get_revision_bundle(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn source_verification(
    State(ctx): State<AppContext>,
    Path(id): Path<systemprompt_identifiers::DependencyVerificationId>,
) -> Result<Json<DependencyVerificationManifest>, ManagedHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .git_verification(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerificationSourceBinding {
    resource_id: systemprompt_identifiers::ManagedResourceId,
    relative_root: String,
}

async fn bind_verification_source(
    State(ctx): State<AppContext>,
    axum::Extension(actor): axum::Extension<systemprompt_models::RequestContext>,
    Path(id): Path<ManagedSourceId>,
    Json(input): Json<VerificationSourceBinding>,
) -> Result<StatusCode, ManagedHttpError> {
    ctx.managed_repository()
        .bind_git_verification_source(
            ctx.system_admin().id(),
            actor.user_id(),
            &GitSourceBinding {
                resource: &input.resource_id,
                source: &id,
                relative_root: &input.relative_root,
            },
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
