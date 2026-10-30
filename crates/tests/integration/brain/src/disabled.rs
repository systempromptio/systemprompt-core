use std::sync::Arc;

use systemprompt_brain::BrainError;
use systemprompt_brain::service::BrainService;
use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{EnteredFact, EnteredStatus, FactScope, SubjectKey};
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_runtime::{BrainCore, BrainInputs, brain_sink};

use crate::events::Landed;
use crate::harness::Harness;
use crate::plane::{OWNERSHIP, kind};

const OUTBOX: &str = "SELECT COUNT(*) FROM event_outbox WHERE consumer = 'brain' AND actor_id = $1";

fn core(h: &Harness, enabled: bool) -> Option<BrainCore> {
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled,
        ..BrainProfile::default()
    };
    BrainCore::new(&BrainInputs {
        database: &h.pool,
        instance_id: &InstanceId::new("brain-disabled"),
        listener_pool: &h.pg,
        notifications: &notifications,
        profile: &profile,
    })
}

#[tokio::test]
async fn a_disabled_brain_composes_nothing_and_its_producers_append_nothing() {
    let h = Harness::new("brain_disabled").await;
    let author = h.user("author", &[]).await;

    let disabled = core(&h, false);
    assert!(disabled.is_none(), "no core, so no handle and no consumer");
    let sink = brain_sink(disabled.as_ref());
    assert!(!sink.is_available());
    let mut tx = h.pg.begin().await.expect("begin");
    assert!(
        sink.emit(&mut tx, &Landed::new().event(&author))
            .await
            .is_err()
    );
    tx.commit().await.expect("commit");
    let service = BrainService::builder(
        (*h.repos).clone(),
        Arc::clone(&sink),
        Arc::new(BrainProfile::default()),
    )
    .build();
    let entered = EnteredFact {
        kind: kind(OWNERSHIP),
        subject: SubjectKey::new("product", "acme"),
        fields: serde_json::Map::from_iter([("team".to_owned(), serde_json::json!("ops"))]),
        scope: FactScope::User(author.clone()),
        topics: Vec::new(),
        status: EnteredStatus::Candidate,
    };
    let refused = service
        .enter(&author, &entered)
        .await
        .expect_err("unavailable");
    assert!(matches!(refused, BrainError::Sink(_)), "{refused:?}");
    assert_eq!(h.count(OUTBOX, author.as_str()).await, 0);

    let enabled = core(&h, true).expect("negative control: an enabled brain composes");
    let mut tx = h.pg.begin().await.expect("begin");
    enabled
        .sink()
        .emit(&mut tx, &Landed::new().event(&author))
        .await
        .expect("emit");
    tx.commit().await.expect("commit");
    assert_eq!(h.count(OUTBOX, author.as_str()).await, 1);
    h.finish().await;
}
