//! Traffic analytics: source, geographic, device, and bot-traffic breakdowns.
//!
//! Defines the [`TrafficCommands`] subcommand tree and the typed output shapes
//! ([`TrafficSourcesOutput`], [`GeoOutput`], [`DevicesOutput`], [`BotsOutput`])
//! rendered by the `analytics traffic` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bots;
mod devices;
mod geo;
mod sources;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum TrafficCommands {
    #[command(about = "Traffic source breakdown", alias = "list")]
    Sources(sources::SourcesArgs),

    #[command(about = "Geographic distribution")]
    Geo(geo::GeoArgs),

    #[command(about = "Device and browser breakdown")]
    Devices(devices::DevicesArgs),

    #[command(about = "Bot traffic analysis")]
    Bots(bots::BotsArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrafficSourceRow {
    pub source: String,
    pub session_count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrafficSourcesOutput {
    pub period: String,
    pub sources: Vec<TrafficSourceRow>,
    pub total_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeoRow {
    pub country: String,
    pub session_count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeoOutput {
    pub period: String,
    pub countries: Vec<GeoRow>,
    pub total_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceRow {
    pub device_type: String,
    pub browser: String,
    pub session_count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DevicesOutput {
    pub period: String,
    pub devices: Vec<DeviceRow>,
    pub total_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BotRow {
    pub bot_type: String,
    pub request_count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BotsOutput {
    pub period: String,
    pub human_sessions: i64,
    pub bot_sessions: i64,
    pub bot_percentage: f64,
    pub bot_breakdown: Vec<BotRow>,
}

pub async fn execute(command: TrafficCommands, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match command {
        TrafficCommands::Sources(args) => {
            let result = sources::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        TrafficCommands::Geo(args) => {
            let result = geo::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        TrafficCommands::Devices(args) => {
            let result = devices::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        TrafficCommands::Bots(args) => {
            let result = bots::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
