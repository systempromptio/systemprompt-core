use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
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
