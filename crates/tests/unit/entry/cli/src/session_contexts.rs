//! A CLI session's conversation context comes only from the admitted
//! member's context seams: with neither provided the session holds no context
//! (`None`); with one seam and not the other the composition is refused with
//! `SessionContextError::Unprovided`; with the reference composition's agent
//! member admitted both are served.

use std::sync::Arc;

use systemprompt_cli::session::{SessionContextError, SessionContexts};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{AppContext as AppContextTrait, BackgroundTasks};

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

#[tokio::test]
async fn session_contexts_are_absent_without_a_provider() {
    let ctx = context().await;
    let kernel = Kernel::new(
        AppContextTrait::database_handle(ctx.as_ref()),
        BackgroundTasks::new(),
        Dependencies::default(),
    );
    let contexts = SessionContexts::from_resolved(&kernel.resolved())
        .expect("a composition without context seams is not an error");
    assert!(contexts.is_none());
}

#[tokio::test]
async fn one_context_seam_without_the_other_is_refused() {
    let ctx = context().await;
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    let directory: systemprompt_traits::DynContextDirectory =
        Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::context::ContextRepository::new(&pool),
        ));
    let kernel = Kernel::new(
        AppContextTrait::database_handle(ctx.as_ref()),
        BackgroundTasks::new(),
        Dependencies::new().with(directory),
    );
    let error = SessionContexts::from_resolved(&kernel.resolved())
        .expect_err("a directory without an issuer is a partial composition");
    assert!(
        matches!(
            error,
            SessionContextError::Unprovided("DynSessionContextIssuer")
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn session_contexts_resolve_from_the_admitted_agent_member() {
    let ctx = context().await;
    let kernel = Kernel::new(
        AppContextTrait::database_handle(ctx.as_ref()),
        BackgroundTasks::new(),
        ctx.kernel().state().clone(),
    );
    let contexts = SessionContexts::from_resolved(&kernel.resolved())
        .expect("negative control: the agent member provides both seams");
    assert!(contexts.is_some());
}
