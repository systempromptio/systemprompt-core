//! The brain's capture consumer, composed for a runner node.
//!
//! Extraction calls models through the in-process gateway client, so a
//! brain model call is metered, guarded, audited and failed over like any
//! other gateway request. The `gateway` member builds that client through
//! its sealed `GatewayOperations` (over the one gateway engines set the
//! process runs, shared with the gateway routes), called here with the
//! kernel's own resolution. The brain handle is the one the `brain` member's
//! slot build returned, started through the sealed `DynBrainConsumerStart`
//! the member provides. An enabled brain on a node without the `gateway`
//! member, or whose gateway configuration is disabled, fails the start rather
//! than leaving the consumer unable to extract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_extension::brain::DynBrainConsumerStart;
use systemprompt_extension::router::DynGatewayOperations;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::RunnerService;

#[derive(Debug, thiserror::Error)]
#[error("brain extraction requires the gateway member, which this node does not compose")]
struct GatewayNotComposed;

pub(super) async fn brain_consumer(
    ctx: &AppContext,
    gateway: Option<&DynGatewayOperations>,
) -> Result<Option<Arc<dyn RunnerService>>> {
    let Some(start) = ctx
        .kernel()
        .state()
        .find::<DynBrainConsumerStart>()
        .cloned()
    else {
        return Ok(None);
    };
    if !ctx
        .kernel()
        .state()
        .get::<BootContext>()?
        .profile()
        .brain
        .enabled
    {
        return Ok(None);
    }
    let gateway = gateway.ok_or(GatewayNotComposed)?;
    let model = gateway
        .brain_model_client(&ctx.kernel().resolved())
        .await
        .context("brain extraction model client")?;
    Ok(start.start(model)?)
}
