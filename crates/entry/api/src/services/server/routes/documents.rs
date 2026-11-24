//! Mounts the routes the `files-http` member's
//! [`FilesSurface`](systemprompt_documents::http::surface::FilesSurface)
//! builds: `/api/v1/documents`, `/api/v1/document-folders` and
//! `/api/v1/uploads`, the anonymous public-link open `/s/{token}` and
//! `/api/v1/issues`; and, separately, `/api/v1/internal/links`. The document
//! routes are absent when `files.documents.enabled` is off, and every route
//! the surface builds is absent when `files-http` is not admitted.
//!
//! The member owns the handlers; this mount owns their layering. Documents
//! and uploads are two-stage groups: the router is protected with the user
//! context and policy, wrapped in its token gate (a download or upload token
//! admits a request before it reaches the user gate), and the wrapped router
//! is declared handler-verified by that token with the documents rate limit
//! outermost. Folders are a plain user declaration. The public-link open is a
//! public declaration: it reads no session and no authz context, and the link
//! token is the credential. Issues are a user declaration under their own
//! content-budget rate scope; triage authorization is the routes' own check
//! on the triage roles or admin. Internal links authenticate by the MCP hop
//! token they are presented, not a session, and are served wherever managed
//! MCP servers run, a worker included.
//!
//! Documents, uploads, folders and the OAuth `/revoke` route are the only
//! routers mounted behind the context middleware that admits a person's UI
//! session (the `ui` audience); every other surface refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_documents::http::surface::{
    DynFilesSurface, FilesRouteState, FilesRoutes, ISSUES_PATH,
};
use systemprompt_documents::links::LinkMinter;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::documents::{DocumentsMountError, files_host_handles, link_tokens};
use crate::routes::internal::links_router;

const DOCUMENTS_RATE: RateLimit = RateLimit::new("documents", RateBudget::Content);
const ISSUES_RATE: RateLimit = RateLimit::new("issues", RateBudget::Content);

fn files_routes(mount: &MountCtx<'_>) -> Result<Option<FilesRoutes>, RouteMountError> {
    let owner = ExtensionId::new(CoreId::FilesHttp.id());
    let kernel = mount.composed.kernel;
    let (Some(standing), Some(surface)) = (
        mount.composed.admitted.built().standing_of(&owner),
        kernel.state().find::<DynFilesSurface>(),
    ) else {
        return Ok(None);
    };
    surface
        .build(FilesRouteState {
            resolved: kernel.route_scoped(&owner, standing, &[]),
            host: files_host_handles(mount.ctx),
        })
        .map(Some)
        .map_err(|source| RouteMountError::initialization("files", source))
}

pub(super) fn mount_files(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let Some(FilesRoutes { documents, issues }) = files_routes(mount)? else {
        return Ok(router);
    };
    router = mount.mount(
        router,
        declare(
            issues,
            ISSUES_PATH,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(ISSUES_RATE),
    )?;
    let Some(documents) = documents else {
        return Ok(router);
    };
    let user_gate = RouteProtection::new(AuthzPolicy::user(), ContextKind::UserUi);
    let group = RouteGroup::ContentAndMisc;

    let gated = documents
        .documents
        .gate(|routes| mount.protect(routes, &user_gate, ApiPaths::DOCUMENTS))?;
    router = mount.mount(
        router,
        declare(
            gated,
            ApiPaths::DOCUMENTS,
            AuthzPolicy::handler_verified(HandlerVerifier::DocumentDownloadToken),
            ContextKind::HandlerResolved,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?;

    let gated = documents
        .uploads
        .gate(|routes| mount.protect(routes, &user_gate, ApiPaths::UPLOADS))?;
    router = mount.mount(
        router,
        declare(
            gated,
            ApiPaths::UPLOADS,
            AuthzPolicy::handler_verified(HandlerVerifier::DocumentUploadToken),
            ContextKind::HandlerResolved,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?;

    router = mount.mount(
        router,
        declare(
            documents.public_link,
            ApiPaths::PUBLIC_DOCUMENT_LINKS,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?;

    Ok(mount.mount(
        router,
        declare(
            documents.folders,
            ApiPaths::DOCUMENT_FOLDERS,
            AuthzPolicy::user(),
            ContextKind::UserUi,
            group,
        )
        .with_rate_limit(DOCUMENTS_RATE),
    )?)
}

pub(super) fn mount_internal_links(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    if !crate::services::server::boot_steps::documents_enabled(mount.ctx) {
        return Ok(router);
    }
    let handles = systemprompt_documents::bound_handles(mount.ctx.kernel().state())
        .map_err(|source| RouteMountError::initialization("internal_links", source))?;
    let documents = handles.service.clone().ok_or_else(|| {
        RouteMountError::initialization("internal_links", DocumentsMountError::Disabled)
    })?;
    let tokens = link_tokens(mount.ctx).ok_or_else(|| {
        RouteMountError::initialization("internal_links", DocumentsMountError::MissingLinkTokens)
    })?;
    let minter = LinkMinter {
        tokens,
        documents,
        uploads: Arc::clone(&handles.uploads),
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
