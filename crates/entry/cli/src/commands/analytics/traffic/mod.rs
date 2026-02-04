mod bots;
mod devices;
mod geo;
mod sources;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum TrafficCommands {
    #[command(about = "Traffic source breakdown")]
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

pub async fn execute(command: TrafficCommands, config: &CliConfig) -> Result<()> {
    match command {
        TrafficCommands::Sources(args) => {
            let result = sources::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Geo(args) => {
            let result = geo::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Devices(args) => {
            let result = devices::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Bots(args) => {
            let result = bots::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub async fn execute_with_pool(
    command: TrafficCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        TrafficCommands::Sources(args) => {
            let result = sources::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Geo(args) => {
            let result = geo::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Devices(args) => {
            let result = devices::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        TrafficCommands::Bots(args) => {
            let result = bots::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
