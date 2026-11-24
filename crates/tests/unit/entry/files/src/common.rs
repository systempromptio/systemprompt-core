//! Shared helpers for the files command tests: command contexts over a
//! database scope or an application context whose kernel holds the state the
//! `core files` commands resolve.

use std::sync::Arc;

use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{
    CliConfig, CommandError, CommandHost, CommandOutput, OutputFormat,
};
use systemprompt_files::{FileRepository, FilesConfig};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, DatabaseContext, Kernel};
use systemprompt_test_fixtures::{fixture_admitted, test_database_url};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle, FileStorage};

pub fn json_cli() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

pub fn with_database(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        json_cli(),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        test_database_url(),
    )
    .with_admitted(Some(fixture_admitted()))
}

pub fn files_kernel(
    pool: &DbPool,
    files_config: Arc<FilesConfig>,
    storage: Arc<dyn FileStorage>,
) -> Kernel {
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(pool);
    Kernel::new(
        handle,
        BackgroundTasks::new(),
        Dependencies::default()
            .with(Arc::new(FileRepository::new(pool)))
            .with(files_config)
            .with(storage),
    )
}

pub fn with_app(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> CommandContext {
    let boot = app.boot().clone();
    boot.install_services_root(boot.require_services_root());
    let admitted = Arc::clone(app.admitted());
    let kernel = files_kernel(
        app.db_pool(),
        systemprompt_test_fixtures::files_config(&app),
        Arc::clone(app.file_storage()),
    );
    CommandContext::with_app_context(cli, env, app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
        .with_kernel(Arc::new(kernel))
}

pub fn rendered(
    host: &CommandContext,
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
