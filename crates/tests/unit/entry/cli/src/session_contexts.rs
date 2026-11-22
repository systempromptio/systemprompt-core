//! A CLI session's conversation context comes only from the admitted
//! member's context seams: with neither provided the session cannot be given
//! a context and creation fails with `SessionContextError::Unprovided`; with
//! the reference composition's agent member admitted both are served.

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
async fn session_contexts_fail_typed_without_a_provider() {
    let ctx = context().await;
    let kernel = Kernel::new(
        AppContextTrait::database_handle(ctx.as_ref()),
        BackgroundTasks::new(),
        Dependencies::default(),
    );
    let error = SessionContexts::from_resolved(&kernel.resolved())
        .expect_err("no member provides the context seams");
    assert!(
        matches!(error, SessionContextError::Unprovided(_)),
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
    SessionContexts::from_resolved(&kernel.resolved())
        .expect("negative control: the agent member provides both seams");
}
