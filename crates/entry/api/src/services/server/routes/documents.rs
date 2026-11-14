//! Mounts `/api/v1/documents`, `/api/v1/document-folders` and
//! `/api/v1/uploads` with their router-scoped state, and, separately,
//! `/api/v1/internal/links`, or nothing when `files.documents.enabled` is
//! off.
//!
//! Documents and uploads are two-stage groups: the router is protected with
//! the user context and policy, wrapped in its token gate (a download or
//! upload token admits a request before it reaches the user gate), and the
//! wrapped router is declared handler-verified by that token with the
//! documents rate limit outermost. Folders are a plain user declaration.
//! Internal links authenticate by the MCP hop token they are presented, not
//! a session, and are served wherever managed MCP servers run, a worker
//! included.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::documents::links::LinkMinter;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::documents::{
    DocumentsState, folders_router, router as documents_router, with_download_tokens,
};
use crate::routes::internal::links_router;
use crate::routes::uploads::{router as uploads_router, with_upload_tokens};

const DOCUMENTS_RATE: RateLimit = RateLimit::new("documents", RateBudget::Content);

pub(super) fn mount_documents(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    if !mount.ctx.files_config().documents_config().is_enabled() {
        return Ok(router);
    }
    let state = DocumentsState::build(mount.ctx)
        .map_err(|source| RouteMountError::initialization("documents", source))?;
    let user_gate = RouteProtection::new(AuthzPolicy::user(), ContextKind::User);
    let group = RouteGroup::ContentAndMisc;

    let documents = with_download_tokens(
        mount.protect(documents_router(state.clone()), &user_gate, ApiPaths::DOCUMENTS)?,
        state.clone(),
    );
    router = mount.mount(
        router,
        declare(
            documents,
            ApiPaths::DOCUMENTS,
            AuthzPolicy::handler_verified(HandlerVerifier::DocumentDownloadToken),
            ContextKind::HandlerResolved,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?;

    let uploads = with_upload_tokens(
        mount.protect(uploads_router(state.clone()), &user_gate, ApiPaths::UPLOADS)?,
        state.clone(),
    );
    router = mount.mount(
        router,
        declare(
            uploads,
            ApiPaths::UPLOADS,
            AuthzPolicy::handler_verified(HandlerVerifier::DocumentUploadToken),
            ContextKind::HandlerResolved,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?;

    Ok(mount.mount(
        router,
        declare(
            folders_router(state),
            ApiPaths::DOCUMENT_FOLDERS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?)
}

pub(super) fn mount_internal_links(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    if !mount.ctx.files_config().documents_config().is_enabled() {
        return Ok(router);
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
    Ok(mount.mount(
        router,
        declare(
            links_router(minter, mount.layers.limits.subject_throttle(per_second)),
            ApiPaths::INTERNAL_LINKS,
            AuthzPolicy::handler_verified(HandlerVerifier::McpHopToken),
            ContextKind::HandlerResolved,
            RouteGroup::InternalLinks,
        ),
    )?)
}
