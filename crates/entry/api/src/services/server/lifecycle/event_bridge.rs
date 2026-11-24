//! Starts the cross-replica event relay the context owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_events::EventBridgeHandle;
use systemprompt_runtime::AppContext;

pub(in crate::services::server) fn start_event_bridge(ctx: &AppContext) -> Result<()> {
    if ctx
        .kernel()
        .state()
        .get::<Arc<EventBridgeHandle>>()?
        .start()
    {
        tracing::info!("Cross-replica event relay started");
    } else {
        tracing::warn!("Event bridge already started; ignoring duplicate start");
    }
    Ok(())
}
