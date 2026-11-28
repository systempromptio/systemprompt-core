//! The scripted instance as a process, for a product that runs outside the
//! test process.
//!
//! It prints one JSON line naming the loopback origin and the personal
//! access token, then serves until interrupted.

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use systemprompt_scripted_instance::{ModelFixture, ModelScript, ScriptedInstance, VerdictScript};

#[derive(Debug, Parser)]
#[command(
    name = "systemprompt-scripted-instance",
    about = "A scripted stand-in for a systemprompt instance and its model"
)]
struct Cli {
    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long)]
    script: Option<PathBuf>,

    #[arg(long)]
    verdicts: Option<PathBuf>,

    #[arg(long = "messages-model", value_name = "MODEL_ID")]
    messages_models: Vec<String>,

    #[arg(long = "chat-model", value_name = "MODEL_ID")]
    chat_models: Vec<String>,

    #[arg(
        long,
        value_name = "TOKENS",
        help = "The context window advertised for every model"
    )]
    context_window: Option<u64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let script = match &cli.script {
        Some(path) => ModelScript::from_file(path)?,
        None => ModelScript::empty(),
    };
    let verdict = match &cli.verdicts {
        Some(path) => VerdictScript::from_file(path)?,
        None => VerdictScript::allow_all(),
    };
    let models = cli
        .messages_models
        .into_iter()
        .map(ModelFixture::messages)
        .chain(cli.chat_models.into_iter().map(ModelFixture::chat))
        .map(|model| match cli.context_window {
            Some(tokens) => model.context_window(tokens),
            None => model,
        });

    let instance = ScriptedInstance::builder()
        .port(cli.port)
        .models(models)
        .verdict(verdict)
        .script(script)
        .start()
        .await
        .context("failed to start the scripted instance")?;

    println!(
        "{}",
        serde_json::json!({
            "base_url": instance.base_url(),
            "pat": instance.pat().as_str(),
        })
    );

    tokio::signal::ctrl_c()
        .await
        .context("failed to listen for the interrupt signal")?;
    instance.shutdown().await?;
    Ok(())
}
