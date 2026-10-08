//! DB-backed tests for archive, restore, legal hold and the guarded purge.

use std::sync::Arc;
use systemprompt_users::{ArchiveParams, UserError, UserRepository, UserService, UserStatus};
use uuid::Uuid;

async fn setup() -> (crate::privacy_fixture::PrivacyFixture, UserService) {
    let fixture = crate::privacy_fixture::PrivacyFixture::new().await;
    let service = UserService::new(Arc::new(UserRepository::new(&fixture.pool)));
    (fixture, service)
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@archive.invalid"),
    )
}

#[tokio::test]
async fn archive_hides_the_user_and_restore_brings_it_back() {
    let (_fixture, service) = setup().await;
    let (name, email) = unique("arch");
    let user = service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    service
        .archive(
            &user.id,
            ArchiveParams {
                archived_by: None,
                reason: Some("left the company"),
                legal_hold: false,
            },
        )
        .await
        .expect("archive");
    let state = service
        .find_archive_state(&user.id)
        .await
        .expect("state")
        .expect("row kept");
    assert!(state.is_archived());
    assert!(state.archived_at.is_some());
    assert_eq!(state.archive_reason.as_deref(), Some("left the company"));
    assert!(
        service
            .find_authenticated_user(&user.id)
            .await
            .expect("lookup")
            .is_none(),
        "an archived user cannot authenticate"
    );

    service.restore(&user.id, 90).await.expect("restore");
    let restored = service
        .find_by_id(&user.id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(restored.status, UserStatus::Active);
    service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn purge_refuses_an_active_user_and_a_legal_hold() {
    let (_fixture, service) = setup().await;
    let (name, email) = unique("hold");
    let user = service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    assert!(matches!(
        service.purge(&user.id).await,
        Err(UserError::NotArchived(_))
    ));
    service
        .archive(
            &user.id,
            ArchiveParams {
                archived_by: None,
                reason: None,
                legal_hold: true,
            },
        )
        .await
        .expect("archive");
    assert!(matches!(
        service.purge(&user.id).await,
        Err(UserError::LegalHold(_))
    ));

    service
        .set_legal_hold(&user.id, false)
        .await
        .expect("release");
    service.purge(&user.id).await.expect("purge");
    assert!(service.find_by_id(&user.id).await.expect("find").is_none());
}

#[tokio::test]
async fn restore_is_refused_after_the_window() {
    let (_fixture, service) = setup().await;
    let (name, email) = unique("late");
    let user = service
        .create(&name, &email, None, None)
        .await
        .expect("create");
    service
        .archive(&user.id, ArchiveParams::default())
        .await
        .expect("archive");

    assert!(matches!(
        service.restore(&user.id, 0).await,
        Err(UserError::RestoreRefused { .. })
    ));
    service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn delete_refuses_a_user_under_legal_hold() {
    let (_fixture, service) = setup().await;
    let (name, email) = unique("held");
    let user = service
        .create(&name, &email, None, None)
        .await
        .expect("create");
    service.set_legal_hold(&user.id, true).await.expect("hold");

    assert!(matches!(
        service.delete(&user.id).await,
        Err(UserError::LegalHold(_))
    ));
    assert!(service.find_by_id(&user.id).await.expect("find").is_some());

    service
        .set_legal_hold(&user.id, false)
        .await
        .expect("release");
    service.delete(&user.id).await.expect("cleanup");
}
