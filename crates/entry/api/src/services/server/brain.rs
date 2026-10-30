//! The brain's capture consumer, composed for a runner node.
//!
//! Extraction calls models through the in-process gateway client, so a
//! brain model call is metered, guarded, audited and failed over like any
//! other gateway request. The client is built here, at the API composition
//! root, from the gateway configuration, catalog, repositories and engines,
//! acting as the `brain` service account (ensured on first start, created by
//! the system admin). An enabled brain on a profile without a gateway fails
//! the start rather than leaving the consumer unable to extract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use systemprompt_extension::brain::BrainModelClient;
use systemprompt_gateway::client::principal::{ServiceIdentity, ServicePrincipal};
use systemprompt_gateway::client::{GatewayClient, GatewayClientParts};
use systemprompt_identifiers::ServiceName;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::brain::BRAIN_ACTOR;
use systemprompt_traits::{AppContext as _, RunnerService};

const BRAIN_SERVICE_EMAIL: &str = "brain@service.invalid";

pub(super) async fn brain_consumer(ctx: &AppContext) -> Result<Option<Arc<dyn RunnerService>>> {
    let Some(brain) = ctx.brain() else {
        return Ok(None);
    };
    let model = brain_model_client(ctx).await?;
    let consumer: Arc<dyn RunnerService> = brain.start_consumer(ctx, model);
    Ok(Some(consumer))
}

async fn brain_model_client(ctx: &AppContext) -> Result<Arc<dyn BrainModelClient>> {
    let gateway = ctx
        .services_config()
        .gateway_config()
        .filter(|gateway| gateway.enabled)
        .cloned()
        .context("brain extraction requires the gateway to be enabled")?;
    let sessions = ctx
        .session_provider()
        .context("brain extraction requires a session provider")?;
    let principal = ServicePrincipal::resolve(
        ctx.user_repository(),
        sessions,
        ServiceIdentity {
            name: ServiceName::try_new(BRAIN_ACTOR)?,
            email: BRAIN_SERVICE_EMAIL.to_owned(),
            created_by: ctx.system_admin().id().clone(),
        },
    )
    .await
    .context("brain service principal")?;
    let extraction = &ctx.profile().brain.extraction;
    let parts = GatewayClientParts {
        config: Arc::new(gateway),
        catalog: Arc::clone(ctx.services_state().catalog()),
        db: Arc::clone(ctx.db_pool()),
        repos: Arc::new(crate::routes::gateway::gateway_repositories(ctx)?),
        engines: Arc::new(crate::routes::gateway::gateway_engines(ctx)?),
        governance: ctx.governance(),
    };
    let mut client = GatewayClient::builder(parts, principal)
        .timeout(Duration::from_millis(extraction.timeout_ms));
    if let Some(model) = &extraction.model {
        client = client.default_model(model.clone());
    }
    Ok(Arc::new(client.build()))
}
