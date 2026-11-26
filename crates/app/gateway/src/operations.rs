//! The `gateway` member's kernel-facing operations.
//!
//! [`GatewayKernelOperations`] implements
//! [`GatewayOperations`]: the member provides it sealed, after its engines,
//! and holds those engines (the one set of breakers, load gauges and upstream
//! client the process runs, shared with the `gateway-http` routes). Kernel
//! code calls it with the kernel's own resolution:
//!
//! - [`access_log`](GatewayOperations::access_log) wraps the whole gateway
//!   surface in the `headers` access-log record
//!   ([`crate::audit::access_log::layer`]);
//! - [`recover_accounting`](GatewayOperations::recover_accounting) settles the
//!   receipts a previous process left in the accounting journal and starts the
//!   periodic recovery pass, and settles nothing when the `ai` member, which
//!   owns `ai_requests`, is not admitted;
//! - [`brain_model_client`](GatewayOperations::brain_model_client) builds the
//!   in-process [`GatewayClient`] the brain's capture consumer extracts
//!   through, acting as the `brain` service account (ensured on first start,
//!   created by the system admin), so a brain model call is metered, guarded,
//!   audited and failed over like any other gateway request. A profile whose
//!   gateway configuration is disabled refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use systemprompt_database::DbPool;
use systemprompt_extension::brain::BrainModelClient;
use systemprompt_extension::prelude::Resolved;
use systemprompt_extension::router::{GatewayOperations, GatewayOperationsError};
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::ServiceName;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_models::brain::BRAIN_ACTOR;
use systemprompt_traits::{
    ConfigWatch, DynContextMaterializer, DynServiceAccounts, DynSessionStore, SessionProvider,
};

use crate::client::principal::{ServiceIdentity, ServicePrincipal};
use crate::client::{GatewayClient, GatewayClientParts};
use crate::registry::GatewayEngines;
use crate::repository::GatewayRepositories;
use crate::surface::GatewayHostHandles;
use crate::surface::repositories::gateway_repositories;

const BRAIN_SERVICE_EMAIL: &str = "brain@service.invalid";

/// The `gateway` member's [`GatewayOperations`].
#[derive(Debug)]
pub struct GatewayKernelOperations {
    engines: Arc<GatewayEngines>,
}

impl GatewayKernelOperations {
    #[must_use]
    pub const fn new(engines: Arc<GatewayEngines>) -> Self {
        Self { engines }
    }
}

fn repositories(kernel: &Resolved<'_>) -> Result<GatewayRepositories, GatewayOperationsError> {
    let host = GatewayHostHandles::resolve(kernel)?;
    Ok(gateway_repositories(
        kernel,
        &host,
        kernel.optional::<DynContextMaterializer>(),
        &kernel.get::<DbPool>()?,
    )?)
}

#[async_trait::async_trait]
impl GatewayOperations for GatewayKernelOperations {
    fn access_log(
        &self,
        kernel: &Resolved<'_>,
        router: Router,
    ) -> Result<Router, GatewayOperationsError> {
        Ok(crate::audit::access_log::layer(
            &kernel.get::<LoggingRuntime>()?,
            router,
        ))
    }

    async fn recover_accounting(
        &self,
        kernel: &Resolved<'_>,
        built: &systemprompt_extension::BuiltState,
    ) -> Result<usize, GatewayOperationsError> {
        if !crate::audit::journal::recovery_admitted(built) {
            tracing::debug!("ai member not admitted; gateway accounting recovery skipped");
            return Ok(0);
        }
        let settlement = repositories(kernel)?.settlement();
        let tasks = kernel.tasks().clone();
        let settled = crate::audit::journal::recover(&settlement)
            .await
            .map_err(|source| GatewayOperationsError::Failed {
                operation: "accounting recovery",
                source: Box::new(source),
            })?;
        crate::audit::journal::spawn_recovery(settlement, &tasks);
        Ok(settled)
    }

    async fn brain_model_client(
        &self,
        kernel: &Resolved<'_>,
    ) -> Result<Arc<dyn BrainModelClient>, GatewayOperationsError> {
        let host = GatewayHostHandles::resolve(kernel)?;
        let Some(gateway) = host
            .services
            .config()
            .gateway_config()
            .filter(|gateway| gateway.enabled)
            .cloned()
        else {
            return Err(GatewayOperationsError::Disabled);
        };
        let sessions: Arc<dyn SessionProvider> = kernel.get::<DynSessionStore>()?;
        let accounts = kernel.get::<DynServiceAccounts>()?;
        let created_by = kernel.get::<SystemAdmin>()?.id().clone();
        let db = kernel.get::<DbPool>()?;
        let governance = kernel.get::<ConfigWatch<GovernanceEngine>>()?.get();
        let repos = repositories(kernel)?;
        let name =
            ServiceName::try_new(BRAIN_ACTOR).map_err(|source| GatewayOperationsError::Failed {
                operation: "brain service name",
                source: Box::new(source),
            })?;
        let principal = ServicePrincipal::resolve(
            accounts.as_ref(),
            sessions,
            ServiceIdentity {
                name,
                email: BRAIN_SERVICE_EMAIL.to_owned(),
                created_by,
            },
        )
        .await
        .map_err(|source| GatewayOperationsError::Failed {
            operation: "brain service principal",
            source: Box::new(source),
        })?;
        let extraction = &host.brain.extraction;
        let parts = GatewayClientParts {
            config: Arc::new(gateway),
            catalog: Arc::clone(host.services.catalog()),
            db,
            repos: Arc::new(repos),
            engines: Arc::clone(&self.engines),
            governance,
        };
        let mut client = GatewayClient::builder(parts, principal)
            .timeout(Duration::from_millis(extraction.timeout_ms));
        if let Some(model) = &extraction.model {
            client = client.default_model(model.clone());
        }
        Ok(Arc::new(client.build()))
    }
}
