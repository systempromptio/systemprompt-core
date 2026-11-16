use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::CliConfig;
use systemprompt_test_fixtures::ensure_test_bootstrap;

pub fn bootstrap_ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    )
    .with_boot(Some(ensure_test_bootstrap().boot.clone()))
}
