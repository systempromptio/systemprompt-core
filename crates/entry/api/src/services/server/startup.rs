//! Early-bind listener that answers health probes before bootstrap finishes.
//!
//! [`bind_and_serve`] binds the TCP listener up front and serves a minimal
//! router — `200 {"status":"starting"}` on the health paths, `503` everywhere
//! else — so platform health checks pass while migrations, content publish, and
//! agent reconciliation run. Once bootstrap completes, the full router is
//! swapped onto the same listener via [`EarlyServer::activate`]; the port is
//! bound exactly once, so probes never hit an unbind/rebind window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, PoisonError, RwLock};
use std::task::{Context, Poll};

use anyhow::{Context as _, Result};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::{StartupEvent, StartupEventExt, StartupEventSender};
use tokio::task::JoinHandle;
use tower::ServiceExt;

#[derive(Debug)]
pub struct EarlyServer {
    swap: Arc<RwLock<Router>>,
    join: JoinHandle<Result<()>>,
    local_addr: SocketAddr,
}

impl EarlyServer {
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn activate(&self, router: Router) {
        *self.swap.write().unwrap_or_else(PoisonError::into_inner) = router;
        tracing::info!("Full API router activated");
    }

    pub async fn join(self) -> Result<()> {
        self.join.await.context("API serve task panicked")?
    }
}

pub async fn bind_and_serve(addr: &str, events: Option<StartupEventSender>) -> Result<EarlyServer> {
    if let Some(ref tx) = events
        && tx
            .unbounded_send(StartupEvent::ServerBinding {
                address: addr.to_owned(),
            })
            .is_err()
    {
        tracing::debug!("Startup event receiver dropped");
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("Failed to read bound address")?;

    if let Some(ref tx) = events {
        tx.server_listening(addr, std::process::id());
    }

    let swap = Arc::new(RwLock::new(starting_router()));
    let outer = Router::new().fallback_service(SwapService {
        swap: Arc::clone(&swap),
    });

    let join = tokio::spawn(async move {
        axum::serve(
            listener,
            outer.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(super::shutdown::shutdown_signal())
        .await
        .map_err(Into::into)
    });

    Ok(EarlyServer {
        swap,
        join,
        local_addr,
    })
}

pub fn starting_router() -> Router {
    Router::new()
        .route(ApiPaths::HEALTH, get(starting_health))
        .route("/health", get(starting_health))
        .fallback(starting_fallback)
}

async fn starting_health() -> impl IntoResponse {
    Json(json!({ "status": "starting" }))
}

async fn starting_fallback() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "service starting" })),
    )
}

#[derive(Clone)]
struct SwapService {
    swap: Arc<RwLock<Router>>,
}

impl tower::Service<Request<Body>> for SwapService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let router = self
            .swap
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(router.oneshot(req))
    }
}
