use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_extension::notifications::{ResourceVisibility, Visibility, VisibilityError};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_models::notifications::{
    AudienceSubject, NotificationEvent, SubjectRef, SubscriptionDecl, SubscriptionTarget,
};
use systemprompt_notifications::NotificationsError;

use crate::harness::{Harness, mention, profile};

#[tokio::test]
async fn a_reader_who_cannot_see_the_document_is_withheld_and_recorded() {
    let h = Harness::new("notif_visibility").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.hidden.hide_from(&reader);

    h.emit(&event).await;
    let drained = h.drain().await;

    assert_eq!(drained.persisted(), 0);
    assert_eq!(drained.outcomes[0].withheld, 1);
    assert!(h.rows_for(&reader).await.is_empty());
    assert!(h.channel.deliveries().is_empty());
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM notification_delivery_failures \
             WHERE stage = 'visibility' AND recipient_user_id = $1",
            reader.as_str(),
        )
        .await,
        1
    );
    h.finish().await;
}

#[tokio::test]
async fn a_document_kind_without_a_visibility_provider_is_withheld() {
    let h = Harness::new("notif_folder").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (mut event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    about(&mut event, "folder");

    h.emit(&event).await;
    let drained = h.drain().await;

    assert_eq!(drained.persisted(), 0);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM notification_delivery_failures \
             WHERE stage = 'visibility' AND recipient_user_id = $1",
            reader.as_str(),
        )
        .await,
        1,
        "a kind without a provider is never assumed visible"
    );
    h.finish().await;
}

struct Stalled;

#[async_trait]
impl ResourceVisibility for Stalled {
    fn kinds(&self) -> &'static [&'static str] {
        &["ledger"]
    }

    async fn visible(
        &self,
        _subject: &SubjectRef,
        _reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        Ok(Visibility::Visible {
            excerpt_allowed: true,
        })
    }
}

fn fast_resolvers() -> NotificationsProfile {
    let mut profile = profile();
    profile.consumer.resolver_timeout_ms = 100;
    profile
}

fn about(event: &mut NotificationEvent, kind: &str) {
    event.subjects = vec![SubjectRef::new(kind, DocumentId::generate().as_str())];
}

#[tokio::test]
async fn a_visibility_provider_that_times_out_withholds_every_reader_it_was_asked_about() {
    let h = Harness::with_seams("notif_visibility_timeout", fast_resolvers(), |parts| {
        parts
            .visibilities
            .push(Arc::new(Stalled) as Arc<dyn ResourceVisibility>);
    })
    .await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (mut event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    about(&mut event, "ledger");

    h.emit(&event).await;
    let started = std::time::Instant::now();
    let drained = h.drain().await;

    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "the stalled provider was not awaited to completion"
    );
    assert_eq!(drained.persisted(), 0);
    assert_eq!(
        h.failures("visibility_timeout").await,
        vec![Some(reader.as_str().to_owned())]
    );
    h.finish().await;
}

#[tokio::test]
async fn any_kind_without_a_visibility_provider_is_withheld() {
    let h = Harness::new("notif_visibility_no_provider").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (mut event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    about(&mut event, "deal");

    h.emit(&event).await;
    let drained = h.drain().await;

    assert_eq!(
        drained.persisted(),
        0,
        "an unprovided kind is never assumed visible"
    );
    assert!(h.channel.deliveries().is_empty());
    assert_eq!(
        h.failures("no_visibility_provider").await,
        vec![Some(reader.as_str().to_owned())]
    );
    h.finish().await;
}

fn resource_subscription(user: &UserId, kind: &str) -> SubscriptionDecl {
    SubscriptionDecl {
        subject: AudienceSubject::User(user.clone()),
        target: SubscriptionTarget::Resource(SubjectRef::new(kind, "r-1")),
        attention_ceiling: None,
        quiet_hours: None,
        digest: None,
    }
}

#[tokio::test]
async fn a_resource_subscription_needs_a_visibility_provider_for_its_kind() {
    let h = Harness::new("notif_visibility_subscribe").await;
    let user = h.user("user", &[]).await;

    let refused = h
        .service
        .subscribe(&user, resource_subscription(&user, "deal"))
        .await
        .expect_err("no provider handles deal");
    assert!(
        matches!(&refused, NotificationsError::UnknownResourceKind { kind } if kind == "deal"),
        "{refused:?}"
    );
    assert!(
        h.service
            .subscriptions(&user)
            .await
            .expect("list")
            .is_empty()
    );

    h.service
        .subscribe(&user, resource_subscription(&user, "document"))
        .await
        .expect("a provided kind is accepted");
    h.finish().await;
}
