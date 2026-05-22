//! Boots the standard `systemprompt` CLI through the facade.
//!
//! Run with: `cargo run -p systemprompt --example cli --features cli -- --help`

use systemprompt::cli::run;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt::init();

    // `run()` parses argv and builds its own config; this mirrors the template's
    // `main.rs`. `CliConfig`/`OutputFormat`/`VerbosityLevel`/`ColorMode` are
    // exposed for embedders that construct settings out-of-band.
    if let Err(err) = Box::pin(run()).await {
        tracing::error!(error = %err, "cli exited with error");
        std::process::exit(1);
    }
}
