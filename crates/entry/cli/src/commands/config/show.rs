use anyhow::Result;
use systemprompt_models::ProfileBootstrap;

use super::types::{
    ConfigOverviewOutput, PathsOverview, RateLimitsSummary, RuntimeOverview, SecurityOverview,
    ServerOverview,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

pub fn execute(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let profile_path = ProfileBootstrap::get_path()?;

    let output = ConfigOverviewOutput {
        profile_name: profile.name.clone(),
        profile_path: profile_path.to_string(),
        server: ServerOverview {
            host: profile.server.host.clone(),
            port: profile.server.port,
            use_https: profile.server.use_https,
            cors_origins_count: profile.server.cors_allowed_origins.len(),
        },
        runtime: RuntimeOverview {
            environment: profile.runtime.environment.to_string(),
            log_level: profile.runtime.log_level.to_string(),
            output_format: profile.runtime.output_format.to_string(),
            no_color: profile.runtime.no_color,
        },
        security: SecurityOverview {
            jwt_issuer: profile.security.issuer.clone(),
            access_token_expiry_seconds: profile.security.access_token_expiration,
            refresh_token_expiry_seconds: profile.security.refresh_token_expiration,
            audiences_count: profile.security.audiences.len(),
        },
        paths: PathsOverview {
            system: profile.paths.system.clone(),
            services: profile.paths.services.clone(),
            bin: profile.paths.bin.clone(),
            web_path: profile.paths.web_path.clone(),
            storage: profile.paths.storage.clone(),
        },
        rate_limits: RateLimitsSummary {
            enabled: !profile.rate_limits.disabled,
            burst_multiplier: profile.rate_limits.burst_multiplier,
            tier_count: 6, // admin, user, a2a, mcp, service, anon
        },
    };

    render_result(&CommandResult::card(output).with_title("Configuration Overview"));

    Ok(())
}
