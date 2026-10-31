use std::sync::Arc;

use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_brain::recall::PlaneDeclaredSource;
use systemprompt_brain::replay::{ProposalReplay, ReplayService};
use systemprompt_brain::service::BrainService;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::notifications::DynNotificationSink;

use crate::harness::Harness;
use crate::jobs_support::RecordedNotices;

fn replay(h: &Harness) -> ProposalReplay {
    ProposalReplay {
        service: Arc::new(ReplayService::new(
            Arc::clone(&h.repos),
            OutboxConsumer::new(h.pg.clone()),
            PipelineSeams::default(),
            h.profile.clone(),
        )),
        model: Arc::clone(&h.model) as _,
    }
}

#[tokio::test]
async fn a_built_service_reports_its_wiring_and_takes_one_replay() {
    let h = Harness::new("brain_service_builder").await;
    let profile = Arc::new(h.profile.clone());
    let notices: DynNotificationSink = Arc::new(RecordedNotices::default());

    let builder = BrainService::builder(
        (*h.repos).clone(),
        Arc::clone(&h.sink),
        Arc::clone(&profile),
    )
    .notifications(notices)
    .declared(Arc::new(PlaneDeclaredSource::new((*h.repos).clone())))
    .audiences(Vec::new());
    let described = format!("{builder:?}");
    assert!(described.contains("notifications: true"), "{described}");
    assert!(described.contains("declared: true"), "{described}");

    let service = builder.build();
    assert!(Arc::ptr_eq(service.profile(), &profile));
    let before = format!("{service:?}");
    assert!(before.contains("enabled: true"), "{before}");
    assert!(before.contains("notifications: true"), "{before}");
    assert!(before.contains("replay: false"), "{before}");

    assert!(service.attach_replay(replay(&h)));
    assert!(
        !service.attach_replay(replay(&h)),
        "only the first replay attaches"
    );
    assert!(format!("{service:?}").contains("replay: true"));
    h.finish().await;
}

#[tokio::test]
async fn a_replay_set_on_the_builder_cannot_be_replaced() {
    let h = Harness::new("brain_service_builder_replay").await;
    let builder = BrainService::builder(
        (*h.repos).clone(),
        Arc::clone(&h.sink),
        Arc::new(h.profile.clone()),
    );
    let described = format!("{builder:?}");
    assert!(described.contains("notifications: false"), "{described}");
    assert!(described.contains("declared: false"), "{described}");

    let service = builder.replay(replay(&h)).build();

    assert!(format!("{service:?}").contains("replay: true"));
    assert!(!service.attach_replay(replay(&h)));
    h.finish().await;
}
