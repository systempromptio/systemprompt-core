use std::sync::Arc;

use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::SyncStatus;
use systemprompt_harness::projection::SkillPublication;
use systemprompt_identifiers::{NativeSessionId, SkillId, UserId};
use tokio::sync::watch;

use systemprompt_harness::sync::poll::Scope;
use systemprompt_harness::sync::runtime::RuntimeFiles;

use crate::sync::fake_ports::FakeStore;
use crate::sync::fake_target::FakeTarget;
use crate::sync::fakes::{BASE, FakeClock, publication};

struct Rig {
    store: FakeStore,
    target: FakeTarget,
    clock: Arc<FakeClock>,
    scope: Scope<FakeStore, FakeTarget>,
    runtime: RuntimeFiles,
    skills: Vec<SkillPublication>,
    _dir: tempfile::TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = FakeStore::default();
    let target = FakeTarget::default();
    let clock = FakeClock::new();
    let scope = Scope {
        account: AccountId::new("a-1"),
        store: store.clone(),
        target: target.clone(),
        instance: store.instance.clone(),
        clock: Arc::<FakeClock>::clone(&clock),
        status: Arc::new(watch::Sender::new(SyncStatus::Idle)),
    };
    let skills = vec![SkillPublication {
        skill: SkillId::new("alpha"),
        dir: "alpha".to_owned(),
        publication: publication(1),
    }];
    let runtime = RuntimeFiles::new(dir.path(), "Harness on WS-01");
    Rig {
        store,
        target,
        clock,
        scope,
        runtime,
        skills,
        _dir: dir,
    }
}

impl Rig {
    async fn capture(&mut self) -> systemprompt_harness::sync::runtime::CaptureOutcome {
        self.runtime
            .capture(&self.scope, &UserId::new("u-1"), &self.skills)
            .await
    }
}

#[tokio::test]
async fn enrols_once_writes_each_publication_and_posts_its_receipt() {
    let mut rig = rig();
    let outcome = rig.capture().await;
    assert!(outcome.complete());
    let instance = rig.store.instance.state();
    assert_eq!(instance.enrolments, 1);
    assert_eq!(instance.plan_calls, 1);
    assert_eq!(instance.receipts.len(), 1);
    assert!(instance.receipts[0].fully_verified());
    drop(instance);
    assert_eq!(rig.target.state().materialized, 1);
    assert_eq!(rig.target.state().device.as_deref(), Some("sp_device_1"));
    assert!(rig.store.state().device.is_some());
    assert!(rig.store.state().outbox.is_some());
}

#[tokio::test]
async fn a_later_capture_fetches_nothing_while_the_files_hold_and_rewrites_them_when_gone() {
    let mut rig = rig();
    rig.capture().await;
    rig.target.state().intact = true;
    assert!(rig.capture().await.complete());
    assert_eq!(rig.store.instance.state().plan_calls, 1);
    assert_eq!(rig.store.instance.state().enrolments, 1);
    rig.target.state().intact = false;
    assert!(rig.capture().await.complete());
    assert_eq!(rig.store.instance.state().plan_calls, 2);
    assert_eq!(rig.target.state().materialized, 2);
}

#[tokio::test]
async fn a_receipt_the_instance_did_not_take_stays_queued_and_goes_first_next_time() {
    let mut rig = rig();
    rig.store.instance.state().receipt_status = Some(503);
    let outcome = rig.capture().await;
    assert!(outcome.undelivered);
    assert_eq!(rig.store.instance.state().receipts.len(), 1);
    rig.store.instance.state().receipt_status = None;
    rig.target.state().intact = true;
    rig.clock.advance(5);
    let outcome = rig.capture().await;
    assert!(outcome.complete());
    assert_eq!(rig.store.instance.state().receipts.len(), 2);
}

#[tokio::test]
async fn a_receipt_refused_for_good_is_not_retried() {
    let mut rig = rig();
    rig.store.instance.state().receipt_status = Some(409);
    assert!(!rig.capture().await.undelivered);
    rig.target.state().intact = true;
    rig.clock.advance(4000);
    rig.capture().await;
    assert_eq!(rig.store.instance.state().receipts.len(), 1);
}

#[tokio::test]
async fn a_refused_device_credential_is_forgotten_and_the_next_capture_enrols_again() {
    let mut rig = rig();
    rig.store.instance.state().plan_status = Some(401);
    assert!(!rig.capture().await.complete());
    assert_eq!(rig.store.state().forgotten, 1);
    assert!(rig.store.state().device.is_none());
    assert_eq!(rig.target.state().device, None);
    rig.store.instance.state().plan_status = None;
    assert!(rig.capture().await.complete());
    assert_eq!(rig.store.instance.state().enrolments, 2);
}

#[tokio::test]
async fn a_refused_enrolment_waits_and_never_fails_the_capture_loudly() {
    let mut rig = rig();
    rig.store.instance.state().enrol_fails = true;
    let outcome = rig.capture().await;
    assert_eq!(outcome.remaining, 1);
    rig.capture().await;
    assert_eq!(rig.store.instance.state().enrolments, 1);
    rig.clock.advance(2);
    rig.capture().await;
    assert_eq!(rig.store.instance.state().enrolments, 2);
}

#[tokio::test]
async fn no_publication_enrols_nothing() {
    let mut rig = rig();
    rig.skills.clear();
    assert!(rig.capture().await.complete());
    assert_eq!(rig.store.instance.state().enrolments, 0);
}

#[tokio::test]
async fn a_session_binds_to_each_acknowledged_receipt_once() {
    let mut rig = rig();
    rig.capture().await;
    let session = NativeSessionId::new("585a6083-9a91-527d-9414-85ec68884f9a");
    assert!(!rig.runtime.bind(&rig.scope, &session).await);
    assert!(!rig.runtime.bind(&rig.scope, &session).await);
    let bindings = rig.store.instance.state().bindings.clone();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].session_id, session);
    assert_eq!(BASE, "https://sp.example.com");
}

#[tokio::test]
async fn without_a_device_no_session_is_queued_and_a_failed_binding_waits() {
    let mut rig = rig();
    let session = NativeSessionId::new("s-1");
    assert!(!rig.runtime.bind(&rig.scope, &session).await);
    assert_eq!(
        rig.store.instance.state().bindings,
        Vec::<systemprompt_models::feedback::receipts::SessionBindingRequest>::new()
    );
    rig.capture().await;
    rig.store.instance.state().binding_status = Some(503);
    assert!(rig.runtime.bind(&rig.scope, &session).await);
    rig.store.instance.state().binding_status = Some(403);
    rig.clock.advance(5);
    assert!(
        !rig.runtime
            .bind(&rig.scope, &NativeSessionId::new("s-2"))
            .await
    );
}
