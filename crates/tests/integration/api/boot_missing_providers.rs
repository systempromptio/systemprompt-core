//! Boot refuses, with a typed `LoaderError::MissingProvider`, when the users
//! extension's session store or IP-ban store is absent from the extension
//! state: the session middleware is not built without a session provider,
//! and the server does not mount without the ban list.

use std::sync::Arc;

use systemprompt_api::services::middleware::SessionMiddleware;
use systemprompt_api::services::server::readiness::ReadinessState;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_database::DbPool;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionStateError,
    LoaderError, StateBuilder,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, error_chain, fixture_app_context_with_extensions, fixture_config,
    test_db_pool,
};
use systemprompt_traits::DynSessionStore;
use systemprompt_users::SessionRepository;

struct SessionsOnly;

impl Extension for SessionsOnly {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "cov-sessions-only",
            name: "Sessions without bans",
            version: "0.0.0",
        }
    }

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        let db = Arc::clone(state.state().get::<DbPool>()?);
        let sessions: DynSessionStore = Arc::new(SessionRepository::new(&db));
        state.provide(sessions)
    }
}

async fn context_with(
    registry: ExtensionRegistry,
) -> anyhow::Result<Arc<systemprompt_runtime::AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    fixture_app_context_with_extensions(
        &pool,
        fixture_config(&bootstrap.database_url),
        registry,
        &[],
    )
}

#[tokio::test]
async fn without_the_users_session_store_the_session_middleware_is_refused() -> anyhow::Result<()>
{
    let ctx = context_with(ExtensionRegistry::new()).await?;
    let err = SessionMiddleware::new(&ctx).expect_err("no session store is provided");
    match &err {
        LoaderError::MissingProvider { source, .. } => {
            assert!(source.type_name().contains("SessionProvider"), "{err}");
        },
        other => panic!("expected MissingProvider, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn without_the_users_session_store_the_server_does_not_mount() -> anyhow::Result<()> {
    let ctx = context_with(ExtensionRegistry::new()).await?;
    let err = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .err()
        .expect("no session store is provided");
    let chain = error_chain(err.as_ref());
    assert!(chain.contains("SessionProvider"), "{chain}");
    Ok(())
}

#[tokio::test]
async fn without_the_users_ban_list_the_server_does_not_mount() -> anyhow::Result<()> {
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(SessionsOnly))?;
    let ctx = context_with(registry).await?;
    let err = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .err()
        .expect("no IP-ban store is provided");
    let chain = error_chain(err.as_ref());
    assert!(chain.contains("IpBanStore"), "{chain}");
    Ok(())
}
