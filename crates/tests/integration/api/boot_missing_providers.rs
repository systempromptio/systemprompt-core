//! Boot refuses, with a typed `LoaderError::MissingProvider`, when the users
//! extension's session store or IP-ban store is absent from the extension
//! state: the session middleware is not built without a session provider,
//! and the server does not mount without the ban list.

use std::sync::Arc;

use systemprompt_api::services::middleware::SessionMiddleware;
use systemprompt_api::services::server::readiness::ReadinessState;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_database::DbPool;
use systemprompt_extension::{Extension, LoaderError, Resolved};
use systemprompt_test_fixtures::{
    Probe, core_members, ensure_test_bootstrap, error_chain, fixture_admitted_of,
    fixture_app_context_with_extensions, fixture_config,
};
use systemprompt_traits::DynSessionStore;
use systemprompt_users::SessionRepository;

fn sessions_only() -> Probe {
    Probe::new("cov-sessions-only", |c| {
        c.provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let sessions: DynSessionStore = Arc::new(SessionRepository::new(&db));
            Ok(sessions)
        });
    })
}

async fn context_without_users(
    extra: Vec<Box<dyn Extension>>,
) -> anyhow::Result<Arc<systemprompt_runtime::AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    let mut members: Vec<Box<dyn Extension>> = core_members()
        .into_iter()
        .filter(|member| member.metadata().id != "users")
        .collect();
    members.extend(extra);
    fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        fixture_admitted_of(members),
    )
    .await
}

#[tokio::test]
async fn without_the_users_session_store_the_session_middleware_is_refused() -> anyhow::Result<()> {
    let ctx = context_without_users(Vec::new()).await?;
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
    let ctx = context_without_users(Vec::new()).await?;
    let err = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
        .expect_err("no session store is provided");
    let chain = error_chain(err.as_ref());
    assert!(chain.contains("SessionProvider"), "{chain}");
    Ok(())
}

#[tokio::test]
async fn without_the_users_ban_list_the_server_does_not_mount() -> anyhow::Result<()> {
    let ctx = context_without_users(vec![Box::new(sessions_only())]).await?;
    let err = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
        .expect_err("no IP-ban store is provided");
    let chain = error_chain(err.as_ref());
    assert!(chain.contains("IpBanStore"), "{chain}");
    Ok(())
}
