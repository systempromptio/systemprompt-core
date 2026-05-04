//! Boots the standard `systemprompt` CLI through the facade.
//!
//! Run with: `cargo run -p systemprompt --example cli --features cli -- --help`

use systemprompt::cli::{CliConfig, ColorMode, OutputFormat, VerbosityLevel, run};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt::init();
    let cfg = CliConfig::default()
        .with_output_format(OutputFormat::Json)
        .with_verbosity(VerbosityLevel::Verbose)
        .with_color_mode(ColorMode::Auto);
    tracing::info!(
        output = ?cfg.output_format,
        verbosity = ?cfg.verbosity,
        color = ?cfg.color_mode,
        "starting CLI"
    );

    if let Err(err) = run().await {
        tracing::error!(error = %err, "cli exited with error");
        std::process::exit(1);
    }
}
