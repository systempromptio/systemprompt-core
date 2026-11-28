//! The scripted instance as a process, for a product that runs outside the
//! test process.
//!
//! It prints one JSON line naming the loopback origin and the personal
//! access token, then serves until interrupted.

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use systemprompt_scripted_instance::{
    ManifestFault, ManifestFixture, ModelFixture, ModelScript, PluginFixture, ScriptedInstance,
    TestPat, VerdictScript,
};

#[derive(Debug, Clone)]
struct SeededDocument {
    path: String,
    file: PathBuf,
}

fn seeded_document(value: &str) -> Result<SeededDocument, String> {
    match value.split_once('=') {
        Some((path, file)) if !path.is_empty() && !file.is_empty() => Ok(SeededDocument {
            path: path.to_owned(),
            file: PathBuf::from(file),
        }),
        _ => Err("expected <PATH>=<FILE>: the Drive path, then the local file".to_owned()),
    }
}

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

    #[arg(
        long,
        value_name = "TEXT",
        help = "The reply to every session-title call, which then takes no turn"
    )]
    title_reply: Option<String>,

    #[arg(
        long,
        value_name = "PLUGIN_ID",
        help = "List a plugin that owns the governance hooks, so verdicts are asked"
    )]
    governance_plugin: Option<String>,

    #[arg(
        long,
        value_enum,
        value_name = "FAULT",
        help = "Serve the manifest with this fault"
    )]
    manifest_fault: Option<ManifestFault>,

    #[arg(
        long,
        value_name = "TEXT",
        help = "The personal access token the instance accepts, in place of the fixture's"
    )]
    pat: Option<String>,

    #[arg(
        long = "document",
        value_name = "PATH=FILE",
        value_parser = seeded_document,
        help = "Start the Drive store with a document at PATH holding the bytes of FILE"
    )]
    documents: Vec<SeededDocument>,
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

    let manifest = match cli.governance_plugin {
        Some(id) => ManifestFixture::minimal().with_plugin(PluginFixture::governance(id)),
        None => ManifestFixture::minimal(),
    };
    let manifest = match cli.manifest_fault {
        Some(fault) => manifest.with_fault(fault),
        None => manifest,
    };
    let mut builder = ScriptedInstance::builder()
        .port(cli.port)
        .manifest(manifest)
        .models(models)
        .verdict(verdict)
        .script(script);
    if let Some(reply) = cli.title_reply {
        builder = builder.title_reply(reply);
    }
    if let Some(pat) = cli.pat {
        builder = builder.pat(TestPat::new(pat));
    }
    for document in cli.documents {
        let bytes = std::fs::read(&document.file)
            .with_context(|| format!("failed to read {}", document.file.display()))?;
        builder = builder.document(document.path, bytes);
    }
    let instance = builder
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
