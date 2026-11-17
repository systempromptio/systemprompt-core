//! Confining the gateway's built-in surface to the kernel's built-in prefixes.
//!
//! [`BUILTIN_GATEWAY_PREFIXES`] is the one list both the surface and the
//! admission check read: `gateway_mount_parts` confines the built-in
//! inference and bridge routers to it with [`confined`] (a request path
//! outside every prefix answers 404, so a built-in route added without its
//! prefix fails its own route tests rather than escaping the check), and the
//! kernel re-check refuses an admitted gateway route whose path falls under
//! one of these prefixes. `/` is the exact gateway root.
//!
//! axum does not let the server enumerate the routes a built `Router` holds,
//! so the bound is enforced on the request: the layer wraps only the routes of
//! the router it is applied to, before any other layer, and paths are matched
//! segment-bounded ([`path_under`]) against the path the router sees, which
//! for the gateway is relative to the gateway base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::IntoResponse;
use systemprompt_extension::reserved::BUILTIN_GATEWAY_PREFIXES;

use crate::services::middleware::path_under;

pub fn confined(router: Router) -> Router {
    router.layer(axum::middleware::from_fn(
        |request: Request, next: Next| async move {
            let path = request.uri().path();
            if BUILTIN_GATEWAY_PREFIXES
                .iter()
                .any(|prefix| path_under(path, prefix))
            {
                next.run(request).await
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        },
    ))
}
