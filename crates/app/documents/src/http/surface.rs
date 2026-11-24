//! The document store's HTTP surface contract.
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
//! does not hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use systemprompt_extension::prelude::{MissingDependency, Resolved};
use systemprompt_files::documents::DocumentsUnbound;
use systemprompt_traits::UserProvider;

pub const ISSUES_PATH: &str = "/api/v1/issues";

/// Builds the document, upload and issue handler routes from the
/// `files-http` route state.
pub trait FilesSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: FilesRouteState<'_>) -> Result<FilesRoutes, FilesSurfaceError>;
}

pub type DynFilesSurface = Arc<dyn FilesSurface>;

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

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum FilesSurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error("the files routes need the document store")]
    Unbound(#[from] DocumentsUnbound),
    #[error("the document routes need a user provider")]
    MissingUserProvider,
}
