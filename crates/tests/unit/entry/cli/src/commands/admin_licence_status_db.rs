use std::sync::Arc;

use systemprompt_cli::admin::licence::{self, LicenceCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::LicenceCheckinRepository;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, test_app_context, test_database_url, test_db_pool,
};

async fn app() -> Arc<AppContext> {
    ensure_test_bootstrap();
    let url = test_database_url();
    let pool = test_db_pool().await;
    test_app_context(&pool, &url)
}

fn ctx(app: &Arc<AppContext>, format: OutputFormat) -> CommandContext {
    let cli = CliConfig::new()
        .with_interactive(false)
        .with_output_format(format);
    crate::boot_ctx::with_app_boot(cli, EnvOverrides::default(), Arc::clone(app))
}

fn repository(app: &AppContext) -> LicenceCheckinRepository {
    LicenceCheckinRepository::new(app.db_pool().pool().as_ref().clone())
}

#[tokio::test]
async fn status_reports_a_recorded_failure_in_every_format() {
    let app = app().await;
    let repo = repository(&app);
    repo.mark_attempt().await.expect("attempt");
    repo.record_failure(Some(503), "licence server unavailable")
        .await
        .expect("failure");
    let state = repo.find_state().await.expect("state").expect("row");
    assert!(state.attempts_total >= 1);
    assert!(state.failures_total >= 1);
    assert!(state.last_attempt_at.is_some());

    for format in [OutputFormat::Json, OutputFormat::Yaml, OutputFormat::Table] {
        licence::execute(LicenceCommands::Status, &ctx(&app, format))
            .await
            .expect("status renders");
    }
}

#[tokio::test]
async fn status_reports_a_recorded_success() {
    let app = app().await;
    let repo = repository(&app);
    repo.mark_attempt().await.expect("attempt");
    repo.record_success(200).await.expect("success");
    let state = repo.find_state().await.expect("state").expect("row");
    assert!(state.last_success_at.is_some());
    licence::execute(LicenceCommands::Status, &ctx(&app, OutputFormat::Json))
        .await
        .expect("status renders");
}
