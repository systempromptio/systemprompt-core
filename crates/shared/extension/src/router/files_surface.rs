//! The document store's contracts the API composes without naming the files
//! capability.
//!
//! The `files-http` member contributes a [`FilesSurface`]: one builder the
//! API mount calls with the route-scoped state of `files-http`
//! ([`FilesRouteState`]). The member owns the handlers; the API owns the
//! layering around what the builder returns. The document and upload routers
//! are two-stage: the API protects [`TokenGatedRoutes::user_routes`] with the
//! user gate, hands the protected router to [`TokenGatedRoutes::token_gate`]
//! (a download or upload link token admits a request before it reaches the
//! user gate) and mounts the result handler-verified with the documents
//! rate limit outermost.
//!
//! [`FilesRouteState::resolved`] is the kernel resolution scoped to
//! `files-http`: it serves the open state and the sealed values conferred on
//! `files-http` (the document link token view) and nothing else sealed.
//! [`FilesHostHandles`] carries the live process handles the kernel state
//! does not hold. [`DriveSurface`] builds the bridge Drive feed the API
//! mounts under the gateway from the same route-scoped resolution.
//!
//! The `documents` member provides, sealed, the one [`DocumentsKernel`] of
//! the process: the `/api/v1/internal/links` router (the API mounts it
//! handler-verified by the MCP hop token and hands it the per-subject
//! [`SubjectCharge`] it charges after verifying the hop), the boot step that
//! registers the document folders as access-control entities, and the
//! `endpoints.documents` block of the root discovery document. Kernel code
//! calls it with the kernel's own resolution, the same authority the API read
//! the document store and the link token authority with before they moved
//! behind this contract. Without the member the API mounts no internal link
//! route (404), runs no folder reconcile and reports the store as disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_traits::{BoxedSource, UserProvider};

use crate::{MissingDependency, Resolved};

/// Builds the document, upload and issue handler routes from the
/// `files-http` route state.
pub trait FilesSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: FilesRouteState<'_>) -> Result<FilesRoutes, FilesSurfaceError>;
}

pub type DynFilesSurface = Arc<dyn FilesSurface>;

/// Builds the bridge Drive feed from the `files-http` route state.
///
/// The API mounts what it builds inside the gateway's traffic; `None` when
/// the document store is disabled or the Drive token view is not conferred,
/// and the feed is then not mounted (404).
pub trait DriveSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, resolved: &Resolved<'_>) -> Result<Option<Router>, FilesSurfaceError>;
}

pub type DynDriveSurface = Arc<dyn DriveSurface>;

pub type ClientIpResolver =
    Arc<dyn Fn(&HeaderMap, Option<&ConnectInfo<SocketAddr>>) -> Option<IpAddr> + Send + Sync>;

/// The state the API mount hands a [`FilesSurface`].
#[derive(Debug)]
pub struct FilesRouteState<'a> {
    pub resolved: Resolved<'a>,
    pub host: FilesHostHandles,
}

/// The live process handles the files surface reads that the kernel state
/// does not hold: the user provider that resolves a caller's current roles
/// and the server's one client-address resolver.
#[derive(Clone)]
pub struct FilesHostHandles {
    pub users: Option<Arc<dyn UserProvider>>,
    pub client_ip: ClientIpResolver,
}

impl std::fmt::Debug for FilesHostHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesHostHandles").finish_non_exhaustive()
    }
}

/// What a [`FilesSurface`] builds: the document routers when the document
/// store is enabled, and the issue router.
#[derive(Debug)]
pub struct FilesRoutes {
    pub documents: Option<DocumentRoutes>,
    pub issues: Router,
}

/// The document store's routers, each still to be protected and mounted by
/// the API.
#[derive(Debug)]
pub struct DocumentRoutes {
    pub documents: TokenGatedRoutes,
    pub uploads: TokenGatedRoutes,
    pub public_link: Router,
    pub folders: Router,
}

pub type TokenGate = Box<dyn FnOnce(Router) -> Router + Send>;

/// A two-stage router: the routes the user gate protects and the token gate
/// that wraps the protected router.
pub struct TokenGatedRoutes {
    pub user_routes: Router,
    pub token_gate: TokenGate,
}

impl TokenGatedRoutes {
    pub fn gate<E>(self, protect: impl FnOnce(Router) -> Result<Router, E>) -> Result<Router, E> {
        Ok((self.token_gate)(protect(self.user_routes)?))
    }
}

impl std::fmt::Debug for TokenGatedRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenGatedRoutes").finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FilesSurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error("the files routes need the document store")]
    Unbound,
    #[error("the document routes need a user provider")]
    MissingUserProvider,
    #[error("the document routes need the document store enabled")]
    Disabled,
    #[error("the document folder entities do not reconcile")]
    Reconcile(#[source] BoxedSource),
    #[error("the documents discovery block does not serialise")]
    Discovery(#[source] serde_json::Error),
}

/// A per-subject limiter a handler charges with a key it derives from a
/// credential it verified; a refusal is a [`ChargeRefused`], which answers
/// 429 with `Retry-After`.
pub trait SubjectCharge: Send + Sync + std::fmt::Debug {
    fn charge(&self, key: &str) -> Result<(), ChargeRefused>;
}

/// A refused [`SubjectCharge::charge`]: the wait before the subject may be
/// charged again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChargeRefused {
    pub retry_after: std::time::Duration,
}

impl IntoResponse for ChargeRefused {
    fn into_response(self) -> Response {
        let wait = self.retry_after;
        let secs = (wait.as_secs() + u64::from(wait.subsec_nanos() > 0)).max(1);
        let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(secs));
        response
    }
}

pub type DynSubjectCharge = Arc<dyn SubjectCharge>;

/// The `documents` member's kernel-facing work: the internal link router,
/// the folder-entity boot step and the discovery block.
///
/// Held as `Arc<dyn DocumentsKernel>` in the kernel state, hence
/// `#[async_trait]`.
#[async_trait::async_trait]
pub trait DocumentsKernel: Send + Sync + std::fmt::Debug {
    fn internal_links(
        &self,
        kernel: &Resolved<'_>,
        throttle: Option<DynSubjectCharge>,
    ) -> Result<Router, FilesSurfaceError>;

    async fn reconcile_folder_entities(
        &self,
        kernel: &Resolved<'_>,
    ) -> Result<u64, FilesSurfaceError>;

    // JSON: the block is merged verbatim into the discovery document, whose
    // wire shape the documents member owns.
    fn discovery(
        &self,
        kernel: &Resolved<'_>,
        base: &str,
    ) -> Result<serde_json::Value, FilesSurfaceError>;
}

pub type DynDocumentsKernel = Arc<dyn DocumentsKernel>;
