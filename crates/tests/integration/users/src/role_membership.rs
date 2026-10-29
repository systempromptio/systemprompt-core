//! Role membership lookups: `is_active_with_role` answers for one user and
//! `list_active_ids_with_role` lists a role's active members; a suspended
//! member is in neither.

use anyhow::Result;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_users::{User, UserRepository, UserStatus};

async fn member(repo: &UserRepository, prefix: &str, role: &str) -> Result<User> {
    let suffix = uuid::Uuid::new_v4().to_string();
    let created = repo
        .create(
            &format!("{prefix}_{}", &suffix[..8]),
            &format!("{prefix}_{suffix}@example.com"),
            None,
            None,
        )
        .await?;
    Ok(repo
        .assign_roles(&created.id, &["user".to_owned(), role.to_owned()])
        .await?)
}

async fn remove(pool: &sqlx::PgPool, users: &[&UserId]) {
    for user in users {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user.as_str())
            .execute(pool)
            .await;
    }
}

#[tokio::test]
async fn role_membership_counts_only_active_holders_of_the_role() -> Result<()> {
    let db = test_db_pool().await;
    let repo = UserRepository::new(&db);
    let pool = db.pool();
    let role = format!(
        "reviewers_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    );

    let active = member(&repo, "rolemember", &role).await?;
    let suspended = member(&repo, "rolesuspended", &role).await?;
    repo.update_status(&suspended.id, UserStatus::Suspended)
        .await?;
    let outsider = member(&repo, "roleoutsider", "user").await?;

    assert!(repo.is_active_with_role(&active.id, &role).await?);
    assert!(
        !repo.is_active_with_role(&outsider.id, &role).await?,
        "a user without the role is not a member"
    );
    assert!(
        !repo.is_active_with_role(&suspended.id, &role).await?,
        "a suspended holder of the role is not a member"
    );
    assert!(
        !repo
            .is_active_with_role(&UserId::new("no_such_user"), &role)
            .await?
    );

    let listed = repo.list_active_ids_with_role(&role, 100).await?;
    assert_eq!(listed, vec![active.id.clone()]);

    remove(pool.as_ref(), &[&active.id, &suspended.id, &outsider.id]).await;
    Ok(())
}
