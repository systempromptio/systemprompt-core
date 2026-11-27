use std::sync::Arc;
use std::time::Duration;

use systemprompt_harness::extension::{ManifestAppliers, SpContext, SpExtension, SpExtensions};
use systemprompt_harness::members::notifications::{
    FeedStatus, NotificationId, NotificationsExtension, NotificationsHandle,
};
use systemprompt_harness::members::slack::{SlackExtension, SlackHandle, SlackStatus};
use systemprompt_traits::{CancellationToken, OwnedTask};

use crate::members::slack::support::{
    FakeInstance, applied, context, enabled, fake_feed, feed_of, id_of,
};

struct Rig {
    _root: tempfile::TempDir,
    _feed: OwnedTask<()>,
    cancel: CancellationToken,
    appliers: ManifestAppliers,
    notifications: Arc<NotificationsHandle>,
    slack: Arc<SlackHandle>,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn rig() -> Rig {
    let root = tempfile::tempdir().unwrap();
    let ctx: SpContext = context(root.path());
    let notifications: Arc<dyn SpExtension> = Arc::new(NotificationsExtension::default());
    let slack: Arc<dyn SpExtension> = Arc::new(SlackExtension::default());
    let extensions = SpExtensions::from_extensions([slack, notifications]).unwrap();
    extensions.provide(&ctx);
    let appliers = extensions.appliers(&ctx);
    let mut services = extensions.services(&ctx);
    assert_eq!(services.len(), 1);
    let service = services.remove(0).item;
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let feed = OwnedTask::spawn("slack-test-feed", async move { service.run(token).await });
    Rig {
        notifications: ctx.handles.get::<NotificationsHandle>().unwrap(),
        slack: ctx.handles.get::<SlackHandle>().unwrap(),
        _root: root,
        _feed: feed,
        cancel,
        appliers,
    }
}

async fn until(
    slack: &SlackHandle,
    done: impl Fn(SlackStatus) -> bool + Send + Sync,
) -> SlackStatus {
    let mut changes = slack.subscribe();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = slack.status();
            if done(status) {
                return status;
            }
            assert!(changes.changed().await);
        }
    })
    .await
    .unwrap()
}

async fn until_live(notifications: &NotificationsHandle) {
    let mut changes = notifications.subscribe();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !matches!(
            changes.borrow_and_update().status(),
            FeedStatus::Live { .. }
        ) {
            changes.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

fn apply(rig: &Rig, fake: &FakeInstance, ids: &[&str]) {
    rig.appliers
        .apply(&applied(&fake.url, Some(enabled(ids))))
        .unwrap();
}

#[test]
fn the_extension_is_named_slack_and_depends_on_notifications() {
    let metadata = SlackExtension::default().metadata();
    assert_eq!(metadata.id.as_str(), "slack");
    assert_eq!(metadata.dependencies, vec![NotificationsExtension::ID]);
    assert!(!metadata.required);
}

#[tokio::test]
async fn slack_is_withheld_until_an_instance_enables_it() {
    let rig = rig();
    assert_eq!(rig.slack.status(), SlackStatus::Withheld);
    assert_eq!(rig.slack.items(), Vec::new());
    assert!(!rig.slack.is_admitted());
}

#[tokio::test]
async fn slack_items_are_filtered_from_the_live_inbox() {
    let feed = feed_of(&["slack.message", "document.shared", "slack.mention"]);
    let fake = fake_feed(&feed).await;
    let rig = rig();
    apply(&rig, &fake, &["notifications", "slack"]);
    let status = until(&rig.slack, |status| {
        matches!(status, SlackStatus::Active { .. })
    })
    .await;
    let SlackStatus::Active {
        items,
        unread,
        latest_at,
    } = status
    else {
        unreachable!("until waited for an active status");
    };
    assert_eq!((items, unread), (2, 2));
    let held = rig.slack.items();
    assert_eq!(latest_at, held[0].created_at);
    let ids: Vec<&str> = held.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec![id_of(0), id_of(2)]);

    let first = NotificationId::try_new(id_of(0)).unwrap();
    rig.notifications.mark_read(vec![first]).await.unwrap();
    let status = until(&rig.slack, |status| {
        matches!(status, SlackStatus::Active { unread: 1, .. })
    })
    .await;
    assert!(matches!(status, SlackStatus::Active { items: 2, .. }));
}

#[tokio::test]
async fn slack_stays_withheld_when_only_notifications_are_enabled() {
    let feed = feed_of(&["slack.message"]);
    let fake = fake_feed(&feed).await;
    let rig = rig();
    apply(&rig, &fake, &["notifications"]);
    until_live(&rig.notifications).await;
    assert_eq!(rig.notifications.snapshot().items().len(), 1);
    assert_eq!(rig.slack.status(), SlackStatus::Withheld);
    assert_eq!(rig.slack.items(), Vec::new());
}

#[tokio::test]
async fn slack_alone_is_withheld_without_notifications() {
    let feed = feed_of(&["slack.message"]);
    let fake = fake_feed(&feed).await;
    let rig = rig();
    apply(&rig, &fake, &["slack"]);
    assert!(!rig.slack.is_admitted());
    assert_eq!(rig.slack.status(), SlackStatus::Withheld);
}

#[tokio::test]
async fn a_live_inbox_without_slack_items_is_quiet() {
    let feed = feed_of(&["document.shared"]);
    let fake = fake_feed(&feed).await;
    let rig = rig();
    apply(&rig, &fake, &["notifications", "slack"]);
    until(&rig.slack, |status| status == SlackStatus::Quiet).await;
    rig.appliers.clear();
    assert_eq!(rig.slack.status(), SlackStatus::Withheld);
}
