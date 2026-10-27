//! Mounts `/api/v1/documents` and `/api/v1/document-folders` with their
//! router-scoped state, the content rate limit and the user authz gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::routes::documents::{
    DocumentsState, folders_router, router as documents_router, with_download_tokens,
};
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount_documents(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
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
            ApiPaths::DOCUMENT_FOLDERS,
            folders_router(state)
                .with_rate_limit(mount.limits, per_second, "documents")?
                .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
        ))
}
