use chrono::Utc;
use systemprompt_identifiers::DocumentId;

use crate::harness::{Harness, mention};

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
    event.subjects = vec![systemprompt_models::notifications::SubjectRef::new(
        "folder",
        DocumentId::generate().as_str(),
    )];

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
        "a protected kind is never assumed visible"
    );
    h.finish().await;
}
