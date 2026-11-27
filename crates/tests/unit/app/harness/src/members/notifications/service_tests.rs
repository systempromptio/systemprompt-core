use std::time::Duration;

use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::SyncErrorKind;
use systemprompt_harness::extension::BackgroundService;
use systemprompt_harness::members::notifications::{
    FeedStatus, NotificationId, NotificationsError, NotificationsHandle, NotificationsService,
    PollCadence,
};
use systemprompt_models::ApiPaths;
use systemprompt_traits::{CancellationToken, OwnedTask};

use crate::members::notifications::support::{
    FakeInstance, client, fake_feed, feed_of, id_of, notification, until,
};

const SLOW: PollCadence = PollCadence {
    interval: Duration::from_secs(3600),
    retry_floor: Duration::from_millis(20),
    retry_ceiling: Duration::from_millis(40),
    not_served_recheck: Duration::from_secs(3600),
};

struct Running {
    cancel: CancellationToken,
    task: OwnedTask<()>,
}

fn run(handle: &NotificationsHandle) -> Running {
    let service = NotificationsService::new(handle.clone(), SLOW);
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let task = OwnedTask::spawn(
        "notifications-test",
        async move { service.run(token).await },
    );
    Running { cancel, task }
}

fn follow(handle: &NotificationsHandle, fake: &FakeInstance) {
    handle.follow(AccountId::new("account-1"), client(fake));
}

fn is_live(status: FeedStatus) -> bool {
    matches!(status, FeedStatus::Live { .. })
}

#[tokio::test]
async fn a_new_handle_is_withheld_and_refuses_actions() {
    let handle = NotificationsHandle::new();
    assert_eq!(handle.status(), FeedStatus::Withheld);
    assert_eq!(handle.snapshot().items(), []);
    let error = handle.mark_all_read().await.unwrap_err();
    assert!(matches!(error, NotificationsError::Withheld));
}

#[tokio::test]
async fn the_service_fills_the_inbox_newest_first_across_pages() {
    let kinds = vec!["document.shared"; 60];
    let feed = feed_of(&kinds, 7);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let running = run(&handle);
    follow(&handle, &fake);
    let inbox = until(&handle, |inbox| is_live(inbox.status())).await;
    assert_eq!(inbox.items().len(), 60);
    assert_eq!(inbox.items()[0].id.as_str(), id_of(0));
    assert_eq!(inbox.items()[59].id.as_str(), id_of(59));
    assert_eq!(inbox.unread().count, 7);
    assert_eq!(inbox.older_cursor(), None);
    let pages = fake.to("GET", ApiPaths::BRIDGE_NOTIFICATIONS);
    assert_eq!(pages.len(), 2);
    assert!(pages[1].query("since").is_some());
    running.cancel.cancel();
    running.task.join().await.unwrap();
}

#[tokio::test]
async fn a_refresh_polls_again_and_merges_new_items() {
    let feed = feed_of(&["document.shared"], 1);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let _running = run(&handle);
    follow(&handle, &fake);
    until(&handle, |inbox| is_live(inbox.status())).await;
    {
        let mut feed = feed.lock().unwrap();
        let mut newer = notification(9, "document.shared");
        newer["created_at"] = serde_json::json!("2026-11-21T10:00:00Z");
        feed.items.insert(0, newer);
        feed.unread = 2;
    }
    handle.refresh();
    let inbox = until(&handle, |inbox| inbox.items().len() == 2).await;
    assert_eq!(inbox.items()[0].id.as_str(), id_of(9));
    assert_eq!(inbox.unread().count, 2);
}

#[tokio::test]
async fn a_failing_feed_backs_off_and_names_its_kind() {
    let feed = feed_of(&[], 0);
    feed.lock().unwrap().fail_with = Some(503);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let _running = run(&handle);
    follow(&handle, &fake);
    let inbox = until(&handle, |inbox| {
        matches!(inbox.status(), FeedStatus::Failing { .. })
    })
    .await;
    let FeedStatus::Failing { kind, retry_in } = inbox.status() else {
        unreachable!("until waited for a failing status");
    };
    assert_eq!(kind, SyncErrorKind::Unreachable);
    assert!(retry_in <= SLOW.retry_ceiling);
    feed.lock().unwrap().fail_with = None;
    until(&handle, |inbox| is_live(inbox.status())).await;
}

#[tokio::test]
async fn an_instance_without_a_feed_is_not_served() {
    let feed = feed_of(&[], 0);
    feed.lock().unwrap().fail_with = Some(404);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let _running = run(&handle);
    follow(&handle, &fake);
    let inbox = until(&handle, |inbox| inbox.status() == FeedStatus::NotServed).await;
    assert_eq!(inbox.items(), []);
}

#[tokio::test]
async fn marking_read_updates_the_inbox_from_the_answer() {
    let feed = feed_of(&["document.shared", "document.shared"], 2);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let _running = run(&handle);
    follow(&handle, &fake);
    until(&handle, |inbox| is_live(inbox.status())).await;
    let first = NotificationId::try_new(id_of(0)).unwrap();
    handle.mark_read(vec![first.clone()]).await.unwrap();
    let inbox = handle.snapshot();
    assert!(inbox.find(&first).unwrap().read_at.is_some());
    assert!(inbox.items()[1].read_at.is_none());
    assert_eq!(inbox.unread().count, 1);
    assert_eq!(handle.mark_seen(vec![first]).await.unwrap(), 1);
}

#[tokio::test]
async fn withholding_empties_the_inbox_and_refuses_actions() {
    let feed = feed_of(&["document.shared"], 1);
    let fake = fake_feed(&feed).await;
    let handle = NotificationsHandle::new();
    let _running = run(&handle);
    follow(&handle, &fake);
    until(&handle, |inbox| is_live(inbox.status())).await;
    handle.withhold();
    let inbox = handle.snapshot();
    assert_eq!(inbox.status(), FeedStatus::Withheld);
    assert_eq!(inbox.items(), []);
    assert_eq!(inbox.unread().count, 0);
    let error = handle.load_older().await.unwrap_err();
    assert!(matches!(error, NotificationsError::Withheld));
}

#[tokio::test]
async fn cancelling_ends_the_service_while_withheld() {
    let handle = NotificationsHandle::new();
    let running = run(&handle);
    running.cancel.cancel();
    tokio::time::timeout(Duration::from_secs(5), running.task.join())
        .await
        .unwrap()
        .unwrap();
}
