use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_api::services::server::runner_supervisor::member_context;
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_traits::{Dependencies, JobContext, RunnerService, StateRequirement};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct OpenSeed;

#[derive(Debug)]
struct SealedHostSeed;

#[derive(Debug)]
struct OwnSealedValue;

#[derive(Debug)]
struct PeerSealedValue;

#[derive(Debug)]
struct Absent;

struct Requiring(Vec<StateRequirement>);

#[async_trait]
impl RunnerService for Requiring {
    fn name(&self) -> &'static str {
        "requiring"
    }

    fn requires(&self) -> Vec<StateRequirement> {
        self.0.clone()
    }

    async fn run(&self, _ctx: JobContext, _shutdown: CancellationToken) {}
}

fn owner() -> ExtensionId {
    ExtensionId::new("runner-owner")
}

fn state() -> Dependencies {
    let mut state = Dependencies::new()
        .with(Arc::new(OpenSeed))
        .with_sealed(Arc::new(SealedHostSeed));
    state
        .provide_sealed(&owner(), Arc::new(OwnSealedValue))
        .expect("own sealed value");
    state
        .provide_sealed(&ExtensionId::new("peer"), Arc::new(PeerSealedValue))
        .expect("peer sealed value");
    state
}

fn admin() -> UserId {
    UserId::new("admin")
}

#[test]
fn a_met_requirement_starts_with_the_member_scoped_context() {
    let service = Requiring(vec![
        StateRequirement::of::<Arc<OpenSeed>>(),
        StateRequirement::of::<OwnSealedValue>(),
    ]);
    let job_ctx = member_context(&state(), &owner(), &service, admin()).expect("requirements met");
    let ctx = job_ctx();
    assert!(ctx.find::<Arc<OpenSeed>>().is_some());
    assert!(ctx.find::<OwnSealedValue>().is_some());
    assert!(ctx.find::<Arc<SealedHostSeed>>().is_none());
    assert!(ctx.find::<PeerSealedValue>().is_none());
}

#[test]
fn an_absent_requirement_refuses_the_start() {
    let service = Requiring(vec![StateRequirement::of::<Absent>()]);
    let Err(refused) = member_context(&state(), &owner(), &service, admin()) else {
        panic!("an absent requirement must refuse the start");
    };
    assert_eq!(refused.service, "requiring");
    assert_eq!(refused.owner, owner());
    assert_eq!(refused.requirement, std::any::type_name::<Absent>());
}

#[test]
fn a_host_sealed_seed_does_not_meet_a_requirement() {
    let service = Requiring(vec![StateRequirement::of::<Arc<SealedHostSeed>>()]);
    assert!(member_context(&state(), &owner(), &service, admin()).is_err());
}

#[test]
fn another_members_sealed_value_does_not_meet_a_requirement() {
    let service = Requiring(vec![StateRequirement::of::<PeerSealedValue>()]);
    assert!(member_context(&state(), &owner(), &service, admin()).is_err());
}
