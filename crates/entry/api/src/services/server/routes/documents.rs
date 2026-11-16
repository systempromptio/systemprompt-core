//! Mounts `/api/v1/documents`, `/api/v1/document-folders` and
//! `/api/v1/uploads` with their router-scoped state, the content rate limit
//! and the user authz gate, and, separately, `/api/v1/internal/links`
//! (authenticated by the MCP hop token it is presented, not a session; served
//! wherever managed MCP servers run, a worker included), or nothing when
//! `files.documents.enabled` is off.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::routes::documents::{
    DocumentsState, folders_router, router as documents_router, with_download_tokens,
};
use crate::routes::internal::links_router;
use crate::routes::uploads::{router as uploads_router, with_upload_tokens};
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;
use systemprompt_runtime::documents::links::LinkMinter;

pub(super) fn mount_documents(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    if !mount.ctx.files_config().documents_config().is_enabled() {
        return Ok(router);
    }
    let state = DocumentsState::build(mount.ctx)
        .map_err(|source| RouteMountError::initialization("documents", source))?;
    let per_second = mount.ctx.config().rate_limits.content_per_second;
    Ok(router
        .nest(
            ApiPaths::DOCUMENTS,
            with_download_tokens(
                documents_router(state.clone())
                    .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
                state.clone(),
            )
            .with_rate_limit(mount.limits, per_second, "documents")?,
        )
        .nest(
            ApiPaths::UPLOADS,
            with_upload_tokens(
                uploads_router(state.clone())
                    .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
                state.clone(),
            )
            .with_rate_limit(mount.limits, per_second, "documents")?,
        )
        .nest(
            ApiPaths::DOCUMENT_FOLDERS,
            folders_router(state)
                .with_rate_limit(mount.limits, per_second, "documents")?
                .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
        ))
}

pub(super) fn mount_internal_links(router: Router, mount: &MountCtx<'_>) -> Router {
    if !mount.ctx.files_config().documents_config().is_enabled() {
        return router;
    }
    let documents = mount.ctx.documents();
    let minter = LinkMinter {
        tokens: mount.ctx.token_authority(),
        documents: Arc::clone(documents.service()),
        uploads: Arc::clone(documents.uploads()),
        services: mount.ctx.services_state().clone(),
        api_external_url: mount.ctx.config().api_external_url.clone(),
    };
    let per_second = mount.ctx.config().rate_limits.content_per_second;
    router.nest(
        ApiPaths::INTERNAL_LINKS,
        links_router(minter, mount.limits.subject_throttle(per_second)),
    )
}
