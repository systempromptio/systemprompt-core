use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::CliConfig;
use systemprompt_test_fixtures::ensure_test_bootstrap;

pub fn bootstrap_ctx() -> CommandContext {
    bootstrap_ctx_with(CliConfig::new().with_interactive(false))
}

pub fn bootstrap_ctx_with(cli: CliConfig) -> CommandContext {
    CommandContext::new(cli, EnvOverrides::default())
        .with_boot(Some(ensure_test_bootstrap().boot.clone()))
}

pub fn new_with_boot(cli: CliConfig, env: EnvOverrides) -> CommandContext {
    CommandContext::new(cli, env).with_boot(Some(ensure_test_bootstrap().boot.clone()))
}

pub fn with_app_boot(
    cli: CliConfig,
    env: EnvOverrides,
    app: std::sync::Arc<systemprompt_runtime::AppContext>,
) -> CommandContext {
    let boot = app.boot().clone();
    CommandContext::with_app_context(cli, env, app).with_boot(Some(boot))
}

pub fn new_on(
    bootstrap: &systemprompt_test_fixtures::TestBootstrap,
    cli: CliConfig,
    env: EnvOverrides,
) -> CommandContext {
    CommandContext::new(cli, env).with_boot(Some(bootstrap.boot.clone()))
}

pub async fn app_on_database(
    bootstrap: &systemprompt_test_fixtures::TestBootstrap,
    database_url: &str,
) -> std::sync::Arc<systemprompt_runtime::AppContext> {
    let mut config = systemprompt_manifest::Config::clone(bootstrap.boot_config());
    config.database_url = database_url.to_owned();
    systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        bootstrap,
        config,
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await
    .expect("fixture app context on the bootstrap's own paths")
}

pub fn core_purge() -> systemprompt_extension::AdmittedPurge {
    systemprompt_extension::AdmittedPurge::of(
        systemprompt_test_fixtures::fixture_admitted().built(),
    )
    .expect("the core purge plan admits")
}
