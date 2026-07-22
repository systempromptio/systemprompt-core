//! DB-backed tests for `UserAdminService` lookup, promotion, and demotion.

use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::{DemoteResult, PromoteResult, UserAdminService, UserService};
use uuid::Uuid;

struct Ctx {
    admin: UserAdminService,
    users: UserService,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let users = UserService::new(&pool).expect("service");
    Some(Ctx {
        admin: UserAdminService::new(users.clone()),
        users,
    })
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@adm.invalid"),
    )
}

#[tokio::test]
async fn find_user_resolves_id_email_and_name() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("lookup");
    let created = ctx
        .users
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let by_id = ctx
        .admin
        .find_user(created.id.as_str())
        .await
        .expect("by id")
        .expect("row");
    assert_eq!(by_id.id, created.id);

    let by_email = ctx
        .admin
        .find_user(&email)
        .await
        .expect("by email")
        .expect("row");
    assert_eq!(by_email.id, created.id);

    let by_name = ctx
        .admin
        .find_user(&name)
        .await
        .expect("by name")
        .expect("row");
    assert_eq!(by_name.id, created.id);

    assert!(
        ctx.admin
            .find_user(&Uuid::new_v4().to_string())
            .await
            .expect("missing uuid")
            .is_none()
    );

    ctx.users.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn promote_grants_admin_then_reports_already_admin() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("promote");
    let created = ctx
        .users
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let promoted = ctx.admin.promote_to_admin(&email).await.expect("promote");
    match promoted {
        PromoteResult::Promoted(user, roles) => {
            assert_eq!(user.id, created.id);
            assert!(roles.contains(&"admin".to_owned()));
            assert!(roles.contains(&"user".to_owned()));
            assert!(user.is_admin());
        },
        other => panic!("expected Promoted, got {other:?}"),
    }

    let again = ctx
        .admin
        .promote_to_admin(&email)
        .await
        .expect("re-promote");
    assert!(matches!(again, PromoteResult::AlreadyAdmin(_)));

    ctx.users.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn demote_removes_admin_and_keeps_user_role() {
    let Some(ctx) = setup().await else {
        return;
    };
    let (name, email) = unique("demote");
    let created = ctx
        .users
        .create(&name, &email, None, None)
        .await
        .expect("create");

    let not_admin = ctx.admin.demote_from_admin(&email).await.expect("noop");
    assert!(matches!(not_admin, DemoteResult::NotAdmin(_)));

    ctx.admin.promote_to_admin(&email).await.expect("promote");
    let demoted = ctx.admin.demote_from_admin(&email).await.expect("demote");
    match demoted {
        DemoteResult::Demoted(user, roles) => {
            assert!(!roles.contains(&"admin".to_owned()));
            assert!(roles.contains(&"user".to_owned()));
            assert!(!user.is_admin());
        },
        other => panic!("expected Demoted, got {other:?}"),
    }

    ctx.users.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn promote_and_demote_report_missing_users() {
    let Some(ctx) = setup().await else {
        return;
    };
    let ghost = format!("ghost-{}@adm.invalid", Uuid::new_v4().simple());
    assert!(matches!(
        ctx.admin.promote_to_admin(&ghost).await.expect("promote"),
        PromoteResult::UserNotFound
    ));
    assert!(matches!(
        ctx.admin.demote_from_admin(&ghost).await.expect("demote"),
        DemoteResult::UserNotFound
    ));
}
