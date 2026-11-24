use crate::{ensure_test_bootstrap, fixture_admitted, AppContextSeeds as _};
use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, CommandDescriptor, GrantedPlane, KernelGrant};
use systemprompt_extension::StandingTag;
use systemprompt_identifiers::ExtensionId;

fn booted(boot: &systemprompt_loader::BootContext) -> systemprompt_loader::BootContext {
    boot.install_services_root(boot.require_services_root());
    boot.clone()
}

pub fn with_database(
    cli: CliConfig,
    env: EnvOverrides,
    db: systemprompt_runtime::DatabaseContext,
    database_url: String,
) -> CommandContext {
    CommandContext::with_database(cli, env, db, database_url)
        .with_admitted(Some(fixture_admitted()))
}

pub fn bootstrap_ctx() -> CommandContext {
    bootstrap_ctx_with(CliConfig::new().with_interactive(false))
}

pub fn bootstrap_ctx_with(cli: CliConfig) -> CommandContext {
    CommandContext::new(cli, EnvOverrides::default())
        .with_boot(Some(booted(&ensure_test_bootstrap().boot)))
        .with_admitted(Some(fixture_admitted()))
}

pub fn new_with_boot(cli: CliConfig, env: EnvOverrides) -> CommandContext {
    CommandContext::new(cli, env)
        .with_boot(Some(booted(&ensure_test_bootstrap().boot)))
        .with_admitted(Some(fixture_admitted()))
}

pub fn with_app_boot(
    cli: CliConfig,
    env: EnvOverrides,
    app: std::sync::Arc<systemprompt_runtime::AppContext>,
) -> CommandContext {
    let boot = booted(app.boot());
    let admitted = std::sync::Arc::clone(app.admitted());
    CommandContext::with_app_context(cli, env, app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
}

pub fn new_on_database(cli: CliConfig, env: EnvOverrides, database_url: &str) -> CommandContext {
    let bootstrap = ensure_test_bootstrap();
    let boot = systemprompt_loader::BootContext::new(
        std::sync::Arc::clone(bootstrap.boot.loaded_profile()),
        std::sync::Arc::clone(bootstrap.boot.secrets()),
    );
    let mut config = systemprompt_manifest::Config::clone(bootstrap.boot_config());
    config.database_url = database_url.to_owned();
    config.database_write_url = None;
    config.database_session_url = None;
    boot.install_config(config)
        .expect("the profile's config over its own database");
    boot.install_services_root(bootstrap.boot.require_services_root());
    CommandContext::new(cli, env)
        .with_boot(Some(boot))
        .with_admitted(Some(fixture_admitted()))
}

pub fn new_on(
    bootstrap: &crate::TestBootstrap,
    cli: CliConfig,
    env: EnvOverrides,
) -> CommandContext {
    CommandContext::new(cli, env)
        .with_boot(Some(booted(&bootstrap.boot)))
        .with_admitted(Some(fixture_admitted()))
}

pub async fn app_reloaded_on_database(
    bootstrap: &crate::TestBootstrap,
    database_url: &str,
) -> std::sync::Arc<systemprompt_runtime::AppContext> {
    crate::fixture_app_context_reloaded_on(bootstrap, database_url, crate::fixture_admitted())
        .await
        .expect("fixture app context over the services tree as the test seeded it")
}

pub fn core_purge() -> systemprompt_extension::AdmittedPurge {
    systemprompt_extension::AdmittedPurge::of(crate::fixture_admitted().built())
        .expect("the core purge plan admits")
}

pub fn rendered(
    host: &CommandContext,
    outcome: anyhow::Result<systemprompt_extension::cli::CommandOutput>,
) -> anyhow::Result<systemprompt_extension::cli::CommandOutput> {
    use systemprompt_extension::cli::{CommandError, CommandHost};
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

pub fn identity(ctx: &CommandContext) -> MemberHost<'_> {
    MemberHost::new(
        ctx,
        CommandDescriptor::FULL
            .with_grant(KernelGrant::SigningKeys)
            .with_grant(KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)),
    )
    .owned_by(ExtensionId::new("identity-http"), StandingTag::Core)
}
