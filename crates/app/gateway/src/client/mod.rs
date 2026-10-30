//! The in-process gateway client: the one sanctioned way for a core service
//! to call a model.
//!
//! A core service never reaches a provider directly. [`GatewayClient`] enters
//! the same [`GatewayService::dispatch`] pipeline the HTTP surface does
//! (policy, upstream resolution, audit, quota reservation, request guards,
//! system-prompt overrides, governance, safety, admission, failover, billing
//! and settlement) without a socket, as a [`ServicePrincipal`]: a service
//! account with its own gateway session, an internal origin and a `purpose`
//! attribution. Every call is therefore metered, guarded, audited and failed
//! over exactly like a user's.
//!
//! The request is already canonical, so [`CanonicalInbound`] never parses a
//! wire body and the upstream body is always encoded from the canonical form;
//! [`decode`] reads the buffered answer back into a [`GatewayCompletion`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod adapter;
#[cfg(feature = "brain")]
pub mod brain;
mod context;
pub mod decode;
pub mod error;
pub mod principal;

pub use adapter::CanonicalInbound;
pub use decode::{CompletionUsage, GatewayCompletion};
pub use error::{DecodeError, GatewayClientError, PrincipalError};
pub use principal::{PURPOSE_DIMENSION, ServiceIdentity, ServicePrincipal};

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use systemprompt_database::DbPool;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::GatewayConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;

use self::context::{ContextInputs, request_context};
use self::decode::DecodeInputs;
use crate::protocol::canonical::CanonicalRequest;
use crate::registry::GatewayEngines;
use crate::repository::GatewayRepositories;
use crate::routing::{RouteSource, resolve_route};
use crate::service::{DispatchInputs, GatewayService};

/// The gateway pipeline's shared state, as the API composition root holds it.
#[derive(Debug)]
pub struct GatewayClientParts {
    pub config: Arc<GatewayConfig>,
    pub catalog: Arc<ResolvedCatalog>,
    pub db: DbPool,
    pub repos: Arc<GatewayRepositories>,
    pub engines: Arc<GatewayEngines>,
    pub governance: Arc<GovernanceEngine>,
}

/// Calls models through the gateway pipeline as a service principal.
#[derive(Debug)]
pub struct GatewayClient {
    config: Arc<GatewayConfig>,
    catalog: Arc<ResolvedCatalog>,
    db: DbPool,
    repos: Arc<GatewayRepositories>,
    engines: Arc<GatewayEngines>,
    governance: Arc<GovernanceEngine>,
    principal: ServicePrincipal,
    default_model: Option<ModelId>,
    timeout: Option<Duration>,
}

#[derive(Debug)]
pub struct GatewayClientBuilder {
    parts: GatewayClientParts,
    principal: ServicePrincipal,
    default_model: Option<ModelId>,
    timeout: Option<Duration>,
}

impl GatewayClientBuilder {
    #[must_use]
    pub fn default_model(mut self, model: ModelId) -> Self {
        self.default_model = Some(model);
        self
    }

    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub fn build(self) -> GatewayClient {
        let GatewayClientParts {
            config,
            catalog,
            db,
            repos,
            engines,
            governance,
        } = self.parts;
        GatewayClient {
            config,
            catalog,
            db,
            repos,
            engines,
            governance,
            principal: self.principal,
            default_model: self.default_model,
            timeout: self.timeout,
        }
    }
}

impl GatewayClient {
    #[must_use]
    pub const fn builder(
        parts: GatewayClientParts,
        principal: ServicePrincipal,
    ) -> GatewayClientBuilder {
        GatewayClientBuilder {
            parts,
            principal,
            default_model: None,
            timeout: None,
        }
    }

    pub const fn principal(&self) -> &ServicePrincipal {
        &self.principal
    }

    pub const fn default_model(&self) -> Option<&ModelId> {
        self.default_model.as_ref()
    }

    pub async fn complete(
        &self,
        request: CanonicalRequest,
        purpose: &'static str,
    ) -> Result<GatewayCompletion, GatewayClientError> {
        let call = Box::pin(self.dispatch(request, purpose));
        match self.timeout {
            Some(limit) => tokio::time::timeout(limit, call)
                .await
                .map_err(|_elapsed| GatewayClientError::Timeout)?,
            None => call.await,
        }
    }

    async fn dispatch(
        &self,
        mut request: CanonicalRequest,
        purpose: &'static str,
    ) -> Result<GatewayCompletion, GatewayClientError> {
        request.stream = false;
        let model = request.model.to_string();
        let resolution = resolve_route(
            RouteSource::Catalog(&self.catalog),
            &request,
            &self.principal.attribution(purpose),
        )
        .map_err(|source| GatewayClientError::Route {
            model: model.clone(),
            source,
        })?;
        let hop = resolution
            .primary()
            .ok_or_else(|| GatewayClientError::NoRoute {
                model: model.clone(),
            })?;
        let session_id = self
            .principal
            .session()
            .await
            .map_err(GatewayClientError::Principal)?;
        let ctx = request_context(ContextInputs {
            principal: &self.principal,
            session_id,
            request: &request,
            hop,
            purpose,
        });
        let decode = DecodeInputs {
            ai_request_id: ctx.ai_request_id.clone(),
            provider: ctx.provider.clone(),
            fallback_model: &model,
            format: request.response_format.clone(),
        };
        let response = Box::pin(GatewayService::dispatch(
            &self.config,
            &self.catalog,
            &self.db,
            &self.repos,
            DispatchInputs {
                request,
                raw_body: Bytes::new(),
                ctx,
                inbound: Arc::new(CanonicalInbound),
                forward_headers: Vec::new(),
                identity_headers: Vec::new(),
                trace_sampled: None,
                governance: Arc::clone(&self.governance),
                engines: Arc::clone(&self.engines),
            },
        ))
        .await
        .map_err(GatewayClientError::from_dispatch)?;
        decode::decode(response, decode).await
    }
}
