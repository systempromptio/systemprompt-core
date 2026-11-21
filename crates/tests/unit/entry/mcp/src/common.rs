//! Shared helpers for the `plugins mcp` command tests: the clap harness, a
//! member host holding the `caller-session` grant the reserved path lists,
//! and command contexts whose kernel holds exactly the values the `mcp`
//! member provides to the commands.

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{
    CliConfig, CommandDescriptor, CommandError, CommandHost, CommandOutput, KernelGrant,
    OutputFormat,
};
use systemprompt_mcp::services::McpServiceObserver;
use systemprompt_mcp_entry::cli::McpCommands;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap, fixture_admitted};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle, McpRegistryProvider};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: McpCommands,
}

pub fn parse(args: &[&str]) -> McpCommands {
    Harness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .command
}

pub fn json_cli() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

pub fn member(ctx: &CommandContext) -> MemberHost<'_> {
    MemberHost::new(
        ctx,
        CommandDescriptor::FULL.with_grant(KernelGrant::CallerSession),
    )
}

pub fn mcp_kernel(app: &AppContext) -> Kernel {
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(app.db_pool());
    let registry = app.mcp_registry().clone();
    let observer = McpServiceObserver::new(
        (**app.service_repository()).clone(),
        Arc::clone(app.app_paths_arc()),
        registry.clone(),
    );
    let provider: Arc<dyn McpRegistryProvider> = Arc::new(registry);
    Kernel::new(
        handle,
        BackgroundTasks::new(),
        Dependencies::default()
            .with(Arc::new(observer))
            .with(provider)
            .with(app.services_state().view())
            .with(Arc::clone(app.app_paths_arc())),
    )
}

pub fn with_app(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> CommandContext {
    let boot = app.boot().clone();
    boot.install_services_root(boot.require_services_root());
    let admitted = Arc::clone(app.admitted());
    let kernel = mcp_kernel(&app);
    CommandContext::with_app_context(cli, env, app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
        .with_kernel(Arc::new(kernel))
}

pub fn with_boot(cli: CliConfig) -> CommandContext {
    let boot = ensure_test_bootstrap().boot.clone();
    boot.install_services_root(boot.require_services_root());
    CommandContext::new(cli, EnvOverrides::default())
        .with_boot(Some(boot))
        .with_admitted(Some(fixture_admitted()))
}

pub async fn app_on_database(bootstrap: &TestBootstrap, database_url: &str) -> Arc<AppContext> {
    let mut config = systemprompt_manifest::Config::clone(bootstrap.boot_config());
    config.database_url = database_url.to_owned();
    systemprompt_test_fixtures::fixture_app_context_on_bootstrap(bootstrap, config, fixture_admitted())
        .await
        .expect("fixture app context on the bootstrap's own paths")
}

pub fn rendered(
    host: &dyn CommandHost,
    outcome: anyhow::Result<CommandOutput>,
) -> anyhow::Result<CommandOutput> {
    match &outcome {
        Ok(output) => host.render(output),
        Err(error) => {
            if let Some(output) = error
                .downcast_ref::<CommandError>()
                .and_then(CommandError::output)
            {
                host.render(output);
            }
        },
    }
    outcome
}

pub fn marked_json(stdout: &str, begin: &str, end: &str) -> serde_json::Value {
    let body = stdout
        .split_once(begin)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(body, _)| body.trim())
        .unwrap_or_else(|| panic!("missing {begin}/{end}: {stdout}"));
    serde_json::from_str(body).unwrap_or_else(|e| panic!("invalid JSON {e}: {body}"))
}
