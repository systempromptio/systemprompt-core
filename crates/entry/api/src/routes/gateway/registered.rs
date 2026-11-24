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
//! caller's [`GatewayPrincipal`] to the request before the handler runs; a
//! request whose credential is missing or rejected never reaches it. The
//! server protects each route with its admitted spec (handler-verified by the
//! gateway credential, its own rate-limit scope and budget with the bearer
//! window) and merges it inside the gateway access log.
//!
//! Only routes admitted here, and the gateway surface's principal-bound
//! routes the gateway mount places behind `verified`, carry a
//! [`GatewayPrincipal`]; nothing else in the server constructs one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::IntoResponse;
use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_extension::{Admitted, RouteProtection};
use systemprompt_runtime::Kernel;

use super::bridge_error::authenticate_bridge;
use crate::kernel::{ApiHttpError, JwtContextExtractor};
use crate::services::middleware::{MountError, gateway_routes};

#[derive(Debug)]
pub(crate) struct RegisteredGatewayRoutes {
    pub(crate) protection: RouteProtection,
    pub(crate) router: Router,
}

pub(crate) fn admitted_gateway_routes(
    admitted: &Admitted,
    kernel: &Kernel,
    jwt: &Arc<JwtContextExtractor>,
) -> Result<Vec<RegisteredGatewayRoutes>, MountError> {
    Ok(gateway_routes(admitted, kernel)?
        .into_iter()
        .map(|mounted| RegisteredGatewayRoutes {
            protection: mounted.protection,
            router: verified(mounted.router, jwt),
        })
        .collect())
}

pub(super) fn verified(router: Router, jwt: &Arc<JwtContextExtractor>) -> Router {
    let jwt = Arc::clone(jwt);
    router.layer(axum::middleware::from_fn(
        move |mut request: Request, next: Next| {
            let jwt = Arc::clone(&jwt);
            async move {
                match authenticate_bridge(&jwt, request.headers()).await {
                    Ok((_claims, user)) => {
                        request
                            .extensions_mut()
                            .insert(GatewayPrincipal::__from_kernel(user.id));
                        next.run(request).await
                    },
                    Err(error) => ApiHttpError::from(error).into_response(),
                }
            }
        },
    ))
}
