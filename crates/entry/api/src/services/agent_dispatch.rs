//! The API's [`AgentDispatch`]: one A2A request through the agent proxy.
//!
//! [`bind_agent_dispatch`] binds it, through the binder the context kept
//! outside the extension state, into the kernel-seeded, sealed
//! `AgentDispatchCell` when the routes are built, beside the messaging
//! dispatcher. The proxy it builds reads the secret store and configuration;
//! the cell is withheld from every member command, job and route state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use systemprompt_agent::models::a2a::{PROTOCOL_VERSION, VERSION_HEADER};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::dispatch::{AgentDispatch, AgentDispatchBinder, AgentDispatchError};
use systemprompt_models::late_bound::AlreadyBound;
use systemprompt_runtime::AppContext;

use crate::kernel::{ProxyKind, ProxyService, ProxyTarget};

const MAX_A2A_RESPONSE_BYTES: usize = 1024 * 1024;

pub struct ApiAgentDispatch {
    ctx: AppContext,
}

impl std::fmt::Debug for ApiAgentDispatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiAgentDispatch").finish_non_exhaustive()
    }
}

impl ApiAgentDispatch {
    #[must_use]
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl AgentDispatch for ApiAgentDispatch {
    async fn send(
        &self,
        agent: &AgentName,
        body: Vec<u8>,
        context: RequestContext,
    ) -> Result<Vec<u8>, AgentDispatchError> {
        let bearer = context
            .auth_token()
            .ok_or(AgentDispatchError::MissingBearer)?
            .as_str()
            .to_owned();
        let mut request = Request::builder()
            .method("POST")
            .uri("/")
            .header(AUTHORIZATION, format!("Bearer {bearer}"))
            .header(CONTENT_TYPE, "application/json")
            .header(VERSION_HEADER, PROTOCOL_VERSION)
            .body(Body::from(body))
            .map_err(|e| AgentDispatchError::Request(Box::new(e)))?;
        request.extensions_mut().insert(context);
        let service_name = ServiceName::new(agent.as_str());
        let target = ProxyTarget {
            service_name: &service_name,
            path: "",
            kind: ProxyKind::Agent,
        };
        let engine = self
            .ctx
            .kernel()
            .state()
            .find::<systemprompt_extension::router::DynProxyBackendResolver>()
            .cloned()
            .map_or_else(ProxyService::new, |backends| {
                ProxyService::new().with_mcp_backends(backends)
            });
        let response = engine
            .proxy_request(target, request, self.ctx.clone())
            .await
            .map_err(|e| AgentDispatchError::Proxy(Box::new(e)))?;
        to_bytes(response.into_body(), MAX_A2A_RESPONSE_BYTES)
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|e| AgentDispatchError::ResponseBody(Box::new(e)))
    }
}

pub fn bind_agent_dispatch(
    ctx: &AppContext,
    binder: AgentDispatchBinder,
) -> Result<(), AlreadyBound> {
    binder.bind(Arc::new(ApiAgentDispatch::new(ctx.clone())))
}
