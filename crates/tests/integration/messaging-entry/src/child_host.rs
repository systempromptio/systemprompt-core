//! A child host (the process `admin mcp serve` and `admin agent-host run`
//! boot) admitting the reference composition with Slack and Teams boots: the
//! API seeds the messaging dispatcher and a child host does not, so the two
//! members withhold their dispatch handles there instead of refusing the
//! boot. The API composition still provides both handles over the dispatcher.

use systemprompt_runtime::{AdmitFn, HostKernel};
use systemprompt_slack_entry::http::{SlackClients, SlackDispatch};
use systemprompt_teams_entry::http::{TeamsClients, TeamsDispatch};
use systemprompt_test_fixtures::{
    ensure_messaging_bootstrap, host_child_env, seed_fixture_system_admin_at,
};

fn reference_with_messaging() -> AdmitFn {
    Box::new(|profile| {
        systemprompt::reference()
            .build(profile)
            .map_err(|e| -> systemprompt_traits::BoxedSource { Box::new(e) })
    })
}

#[tokio::test]
async fn a_child_host_admitting_slack_and_teams_boots_without_their_dispatch_handles() {
    let b = ensure_messaging_bootstrap();
    seed_fixture_system_admin_at(&b.database_url)
        .await
        .expect("seed the system admin");
    let env = host_child_env(b);
    let parts = HostKernel::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        reference_with_messaging(),
    )
    .await
    .unwrap_or_else(|e| panic!("a child host with Slack and Teams admitted boots: {e:#}"));
    let state = parts.kernel_state();
    assert!(state.find::<SlackClients>().is_some());
    assert!(state.find::<TeamsClients>().is_some());
    assert!(state.find::<SlackDispatch>().is_none());
    assert!(state.find::<TeamsDispatch>().is_none());
}

#[tokio::test]
async fn the_api_composition_provides_both_dispatch_handles() {
    let b = ensure_messaging_bootstrap();
    let ctx = super::common::messaging_context(&b.database_url).await;
    let state = ctx.kernel().state();
    assert!(state.find::<SlackDispatch>().is_some());
    assert!(state.find::<TeamsDispatch>().is_some());
}
