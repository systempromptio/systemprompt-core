//! Boot refuses when the users member's session store or IP-ban store is
//! absent: without the session store the kernel refuses the composition and
//! names the store; with only the session store provided the server does not
//! mount without the ban list.

use std::sync::Arc;

use systemprompt_api::services::server::readiness::ReadinessState;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_database::DbPool;
use systemprompt_extension::{Extension, Resolved};
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, error_chain, fixture_admitted_of, fixture_core_members,
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
    let mut members: Vec<Box<dyn Extension>> = fixture_core_members()
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
async fn without_the_users_session_store_the_composition_is_refused_naming_the_store()
-> anyhow::Result<()> {
    let err = context_without_users(Vec::new())
        .await
        .expect_err("no session store is provided");
    let chain = format!("{err:#}");
    assert!(chain.contains("SessionStore"), "{chain}");
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
