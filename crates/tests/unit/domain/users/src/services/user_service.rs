//! DB-backed tests for `UserService` create/lookup/update/delete operations.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::{UpdateUserParams, UserService, UserStatus};
use uuid::Uuid;

struct Ctx {
    service: UserService,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let service = UserService::new(&pool).expect("service");
    Some(Ctx { service })
}

async fn delete_user(ctx: &Ctx, id: &UserId) {
    let _ = ctx.service.delete(id).await;
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    let name = format!("{prefix}-{tag}");
    let email = format!("{prefix}-{tag}@svc.invalid");
    (name, email)
}

#[tokio::test]
async fn create_then_find_by_id_email_name() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("usvc");

    let created = ctx
        .service
        .create(&name, &email, Some("Full Name"), Some("Disp"))
        .await
        .expect("create");
    assert_eq!(created.name, name);
    assert_eq!(created.email, email);
    assert_eq!(created.full_name.as_deref(), Some("Full Name"));

    let by_id = ctx
        .service
        .find_by_id(&created.id)
        .await
        .expect("find_by_id")
        .expect("present");
    assert_eq!(by_id.id, created.id);

    let by_email = ctx
        .service
        .find_by_email(&email)
        .await
        .expect("find_by_email")
        .expect("present");
    assert_eq!(by_email.id, created.id);

    let by_name = ctx
        .service
        .find_by_name(&name)
        .await
        .expect("find_by_name")
        .expect("present");
    assert_eq!(by_name.id, created.id);

    delete_user(&ctx, &created.id).await;
}

#[tokio::test]
async fn find_by_id_unknown_returns_none() {
    let Some(ctx) = setup().await else {
        return;
    };
    let missing = UserId::new(format!("missing-{}", Uuid::new_v4()));
    let found = ctx.service.find_by_id(&missing).await.expect("find_by_id");
    assert!(found.is_none());
}

#[tokio::test]
async fn update_fields_persist() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("uupd");
    let created = ctx
        .service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let new_email = format!("changed-{}@svc.invalid", Uuid::new_v4().simple());
    let updated = ctx
        .service
        .update_email(&created.id, &new_email)
        .await
        .expect("update_email");
    assert_eq!(updated.email, new_email);

    let renamed = ctx
        .service
        .update_full_name(&created.id, "Renamed Person")
        .await
        .expect("update_full_name");
    assert_eq!(renamed.full_name.as_deref(), Some("Renamed Person"));

    let verified = ctx
        .service
        .update_email_verified(&created.id, true)
        .await
        .expect("update_email_verified");
    assert_eq!(verified.email_verified, Some(true));

    let suspended = ctx
        .service
        .update_status(&created.id, UserStatus::Suspended)
        .await
        .expect("update_status");
    assert_eq!(
        suspended.status.as_deref(),
        Some(UserStatus::Suspended.as_str())
    );

    delete_user(&ctx, &created.id).await;
}

#[tokio::test]
async fn update_all_fields_replaces_state() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("uall");
    let created = ctx
        .service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let new_email = format!("all-{}@svc.invalid", Uuid::new_v4().simple());
    let updated = ctx
        .service
        .update_all_fields(
            &created.id,
            UpdateUserParams {
                email: &new_email,
                full_name: Some("All Fields"),
                display_name: Some("AllDisp"),
                status: UserStatus::Active,
            },
        )
        .await
        .expect("update_all_fields");
    assert_eq!(updated.email, new_email);
    assert_eq!(updated.full_name.as_deref(), Some("All Fields"));
    assert_eq!(updated.display_name.as_deref(), Some("AllDisp"));

    delete_user(&ctx, &created.id).await;
}

#[tokio::test]
async fn assign_roles_persists() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("urole");
    let created = ctx
        .service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let roles = vec!["admin".to_owned(), "user".to_owned()];
    let updated = ctx
        .service
        .assign_roles(&created.id, &roles)
        .await
        .expect("assign_roles");
    assert!(updated.roles.contains(&"admin".to_owned()));

    delete_user(&ctx, &created.id).await;
}

#[tokio::test]
async fn delete_removes_user() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("udel");
    let created = ctx
        .service
        .create(&name, &email, None, None)
        .await
        .expect("create");

    ctx.service.delete(&created.id).await.expect("delete");
    assert!(
        ctx.service
            .find_by_id(&created.id)
            .await
            .expect("find_by_id")
            .is_none()
    );
}

#[tokio::test]
async fn create_anonymous_then_flagged_temporary() {
    let Some(ctx) = setup().await else {
        return;
    };
    let fingerprint = format!("fp-{}", Uuid::new_v4());
    let anon = ctx
        .service
        .create_anonymous(&fingerprint)
        .await
        .expect("create_anonymous");

    let is_temp = ctx
        .service
        .is_temporary_anonymous(&anon.id)
        .await
        .expect("is_temporary_anonymous");
    assert!(is_temp);

    delete_user(&ctx, &anon.id).await;
}
