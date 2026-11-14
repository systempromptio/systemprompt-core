//! The `agent_runs_api` extension's declaration of `/api/v1/agent-runs`: the
//! user policy and context, the `agent_runs` tasks-budget rate limit, the
//! `Agent` route group and no body limit above the default, read from the
//! declaration the API server mounts. The run repository and the A2A run
//! launcher it depends on come from the `agent_runs` extension's `provide`.

use systemprompt_agent_runs::AgentRunRepository;
use systemprompt_agent_runs_entry::http::AgentRunsApiExtension;
use systemprompt_extension::{
    ContextKind, Extension, RateBudget, RateLimit, RouteGroup, StateRequirement,
};
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::offline_extension_context;
use systemprompt_traits::DynAgentRunLauncher;

#[test]
fn the_extension_requires_the_run_repository_agent_runs_provides() {
    let extension = AgentRunsApiExtension;
    assert_eq!(extension.metadata().id, "agent_runs_api");
    assert_eq!(
        extension.requires(),
        vec![StateRequirement::of::<AgentRunRepository>()]
    );
}

#[tokio::test]
async fn the_agent_runs_extension_provides_the_repository_and_the_launcher() {
    let ctx = offline_extension_context();
    let state = ctx.extension_state();
    assert!(state.find::<AgentRunRepository>().is_some());
    assert!(state.find::<DynAgentRunLauncher>().is_some());
}

#[tokio::test]
async fn the_route_group_is_declared_user_gated_and_rate_limited() {
    let ctx = offline_extension_context();
    let declared = AgentRunsApiExtension
        .router(&*ctx)
        .expect("the routes are declared over a state with the run repository");
    assert_eq!(declared.base_path, ApiPaths::AGENT_RUNS);
    assert_eq!(declared.group, RouteGroup::Agent);
    assert_eq!(declared.protection.context, ContextKind::User);
    assert_eq!(
        declared.protection.policy.allowed(),
        &[UserType::User, UserType::Admin]
    );
    assert!(declared.protection.policy.handler_verifier().is_none());
    assert_eq!(
        declared.protection.rate_limit,
        Some(RateLimit::new("agent_runs", RateBudget::Tasks))
    );
    assert_eq!(declared.protection.body_limit, None);
    assert!(declared.protection.layers.is_empty());
    assert!(declared.protection.frame_options.is_none());
}
