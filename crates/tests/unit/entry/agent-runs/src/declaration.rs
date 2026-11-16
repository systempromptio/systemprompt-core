//! The `agent_runs_api` member's contribution of `/api/v1/agent-runs`: one
//! route set at `Api(AgentRuns)` in the `Agent` group, every route behind the
//! user policy and context, the `agent_runs` tasks-budget rate limit and the
//! default body limit — the shape the reference composition confers on
//! `agent_runs_api` — and the run repository it requires from `agent_runs`.

use std::any::TypeId;

use systemprompt_agent_runs::AgentRunRepository;
use systemprompt_agent_runs_entry::extension::AgentRunsApiExtension;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::router::{ApiBase, BasePath, BodyLimit};
use systemprompt_extension::{
    ContextKind, Contributions, Extension, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::UserType;

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("agent_runs_api"), &mut sink);
    AgentRunsApiExtension
        .contribute(&mut c)
        .expect("the agent_runs_api member contributes");
    sink
}

#[test]
fn the_member_requires_the_run_repository_agent_runs_provides() {
    assert_eq!(AgentRunsApiExtension.metadata().id, "agent_runs_api");
    let sink = contributed();
    let required: Vec<TypeId> = sink.requires().iter().map(|r| r.type_id()).collect();
    assert_eq!(required, [TypeId::of::<AgentRunRepository>()]);
}

#[test]
fn the_route_set_is_user_gated_and_rate_limited_at_agent_runs() {
    let sink = contributed();
    let sets = sink.routes();
    assert_eq!(sets.len(), 1);
    let set = &sets[0];
    assert_eq!(set.base(), BasePath::Api(ApiBase::AgentRuns));
    assert_eq!(set.group(), RouteGroup::Agent);
    assert_eq!(set.defs().len(), 7);
    for def in set.defs() {
        assert_eq!(def.spec.context, ContextKind::User);
        assert_eq!(
            def.spec.policy.allowed(),
            &[UserType::User, UserType::Admin]
        );
        assert!(def.spec.policy.handler_verifier().is_none());
        assert_eq!(
            def.spec.limits.rate,
            RateLimit::new("agent_runs", RateBudget::Tasks)
        );
        assert_eq!(def.spec.limits.body, BodyLimit::Default);
    }
}

#[test]
fn the_member_contributes_no_command() {
    assert!(contributed().commands().is_empty());
}
