//! Starts the live OTLP export the profile names, before the server serves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;

use systemprompt_runtime::AppContext;
use systemprompt_traits::BackgroundTasks;

pub(super) fn start_otlp_export(ctx: &AppContext) -> Result<()> {
    let state = ctx.kernel().state();
    let Some(config) = state
        .get::<systemprompt_loader::BootContext>()?
        .profile()
        .observability
        .otlp()
    else {
        return Ok(());
    };
    if state
        .get::<systemprompt_logging::LoggingRuntime>()?
        .sink()
        .start_otlp_export(config, state.get::<BackgroundTasks>()?)
    {
        tracing::info!(endpoint = %config.endpoint, "OTLP export started");
    }
    Ok(())
}
