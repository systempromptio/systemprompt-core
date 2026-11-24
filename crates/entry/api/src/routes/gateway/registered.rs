//! Capability route groups admitted onto the gateway surface.
//!
//! A core member that serves part of the bridge-facing surface (the bridge
//! inbox) contributes it with `Contributions::gateway_route` under a gateway
//! path conferral. The facade admits it and the kernel re-check refuses
//! anything that is not a listed gateway-route shape, a route under a
//! built-in gateway prefix, or a disabled member's route
//! ([`gateway_routes`]). What this module adds is the gateway's own
//! verification: every admitted route is placed behind the gateway
//! credential check, which authenticates the bridge bearer and attaches the
//! caller's
//! [`GatewayPrincipal`] to
//! the request before the handler runs; a
//! request whose credential is missing or rejected never reaches it. The
//! server protects each route with its admitted spec (handler-verified by the
//! gateway credential, its own rate-limit scope and budget with the bearer
//! window) and merges it inside the gateway access log.
//!
//! Only routes admitted here, and the gateway surface's principal-bound
//! routes the gateway mount places behind `verified`, carry a
//! [`GatewayPrincipal`],
//! stamped by `verified` from a bearer the kernel's request authenticator
//! accepted; nothing else in the server constructs one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::IntoResponse;
use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_extension::{Admitted, RouteProtection};
use systemprompt_identifiers::JwtToken;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_runtime::Kernel;
use systemprompt_security::extraction::extract_credential;

use crate::kernel::ApiHttpError;
use crate::services::middleware::{MountError, gateway_routes};

#[derive(Debug)]
pub(crate) struct RegisteredGatewayRoutes {
    pub(crate) protection: RouteProtection,
    pub(crate) router: Router,
}

pub(crate) fn admitted_gateway_routes(
    admitted: &Admitted,
    kernel: &Kernel,
    authenticator: &DynRequestAuthenticator,
) -> Result<Vec<RegisteredGatewayRoutes>, MountError> {
    Ok(gateway_routes(admitted, kernel)?
        .into_iter()
        .map(|mounted| RegisteredGatewayRoutes {
            protection: mounted.protection,
            router: verified(mounted.router, authenticator),
        })
        .collect())
}

pub(super) fn verified(router: Router, authenticator: &DynRequestAuthenticator) -> Router {
    let authenticator = Arc::clone(authenticator);
    router.layer(axum::middleware::from_fn(
        move |mut request: Request, next: Next| {
            let authenticator = Arc::clone(&authenticator);
            async move {
                let headers = request.headers().clone();
                match principal(&authenticator, &headers).await {
                    Ok(principal) => {
                        request.extensions_mut().insert(principal);
                        next.run(request).await
                    },
                    Err(error) => error.into_response(),
                }
            }
        },
    ))
}

async fn principal(
    authenticator: &DynRequestAuthenticator,
    headers: &HeaderMap,
) -> Result<GatewayPrincipal, ApiHttpError> {
    let credential = extract_credential(headers).ok_or_else(|| {
        ApiHttpError::from(
            ApiError::unauthorized("Missing Authorization or x-api-key credential")
                .with_error_key("missing_credential"),
        )
    })?;
    let (_claims, user) = authenticator
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|source| {
            ApiHttpError::from(
                ApiError::unauthorized("Bridge credential rejected")
                    .with_error_key("invalid_credential")
                    .with_source(source),
            )
        })?;
    Ok(GatewayPrincipal::__from_kernel(user.id))
}
