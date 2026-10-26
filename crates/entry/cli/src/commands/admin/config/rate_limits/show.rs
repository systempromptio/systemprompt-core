//! `admin config rate-limits show` command with effective limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_logging::CliService;

use crate::cli_settings::OutputFormat;
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

use super::super::types::RateLimitsOutput;

pub(super) fn execute_show(ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile = ctx.profile()?;
    let limits = &profile.rate_limits;

    let output = RateLimitsOutput {
        disabled: limits.disabled,
        oauth_public_per_second: limits.oauth_public_per_second,
        oauth_auth_per_second: limits.oauth_auth_per_second,
        contexts_per_second: limits.contexts_per_second,
        tasks_per_second: limits.tasks_per_second,
        artifacts_per_second: limits.artifacts_per_second,
        agent_registry_per_second: limits.agent_registry_per_second,
        agents_per_second: limits.agents_per_second,
        mcp_registry_per_second: limits.mcp_registry_per_second,
        mcp_per_second: limits.mcp_per_second,
        stream_per_second: limits.stream_per_second,
        content_per_second: limits.content_per_second,
        gateway_per_second: limits.gateway_per_second,
        burst_multiplier: limits.burst_multiplier,
    };

    render_result(&CommandOutput::card_value("Rate Limits", &output), config);

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}
