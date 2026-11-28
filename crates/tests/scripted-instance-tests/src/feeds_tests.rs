use std::time::Duration;

use systemprompt_harness::extension::BackgroundService as _;
use systemprompt_harness::members::brain::{BrainClient, BrainProbe, BrainService, BrainStatus};
use systemprompt_harness::members::notifications::{
    FeedQuery, NotificationsClient, NotificationsError,
};
use systemprompt_scripted_instance::ModelScript;
use systemprompt_traits::CancellationToken;

use crate::fixture::{Signed, http};

const PROBE_DEADLINE: Duration = Duration::from_secs(30);

#[tokio::test(flavor = "multi_thread")]
async fn the_brain_probe_settles_on_enabled_or_disabled() {
    let signed = Signed::start(ModelScript::empty()).await;
    let service = BrainService::new(http());
    let mut feed = service.subscribe();
    service.attach(BrainClient::new(
        &signed.client,
        http(),
        signed.accounts.credentials(&signed.account),
    ));
    let cancel = CancellationToken::new();
    let probe = BrainProbe::new(service.clone());
    let stop = cancel.clone();
    let task = tokio::spawn(async move { probe.run(stop).await });

    let settled = tokio::time::timeout(PROBE_DEADLINE, async {
        loop {
            let status = feed.borrow_and_update().status;
            if matches!(status, BrainStatus::Enabled | BrainStatus::Disabled) {
                return status;
            }
            feed.changed().await.unwrap();
        }
    })
    .await;
    cancel.cancel();
    task.await.unwrap();
    let status = settled.expect("the brain probe did not settle");
    assert_eq!(
        status,
        BrainStatus::Disabled,
        "the scripted instance serves no brain"
    );
    assert_eq!(service.status(), status);
    signed.accounts.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_notifications_feed_is_served_or_reported_not_served() {
    let signed = Signed::start(ModelScript::empty()).await;
    let client = NotificationsClient::new(
        &signed.client,
        http(),
        signed.accounts.credentials(&signed.account),
    );
    match client.feed(&FeedQuery::newest(1)).await {
        Err(NotificationsError::NotServed { endpoint }) => {
            assert!(endpoint.ends_with("/notifications"), "{endpoint}");
        },
        other => panic!("the scripted instance serves no feed, yet: {other:?}"),
    }
    signed.accounts.shutdown();
}
