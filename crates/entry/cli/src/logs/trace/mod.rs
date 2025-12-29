mod ai_artifacts;
mod ai_display;
mod ai_mcp;
mod ai_trace;
mod client;
mod display;
mod json;
mod viewer;

use anyhow::Result;
use clap::Subcommand;

pub use viewer::TraceOptions;

#[derive(Subcommand)]
pub enum TraceCommands {
    #[command(about = "View trace for a message or trace ID")]
    View {
        trace_id: Option<String>,
        #[command(flatten)]
        options: TraceOptions,
    },

    #[command(about = "AI task trace - inspect task execution details")]
    Ai(ai_trace::AiTraceOptions),
}

pub async fn execute(command: TraceCommands) -> Result<()> {
    match command {
        TraceCommands::View { trace_id, options } => {
            viewer::execute(trace_id.as_deref(), options).await
        },
        TraceCommands::Ai(options) => ai_trace::execute(options).await,
    }
}
