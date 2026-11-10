//! The SSO sign-in path against the real users repository: an email clash
//! refuses unless the provider links by email, privileged and suspended
//! accounts are never linked, and the directory's roles land only on an
//! account the provider provisioned.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_oauth_issuance::sso::{FinishOutcome, SsoDriver};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_users::{NewServiceAccount, User, UserRepository, UserService, UserStatus};

use super::providers::{OPEN, OPEN_ISSUER, RESOLVED_ROLE, UNLINKED};
use super::stores::CountingSessions;
use super::{driver_on, finish, refusal_reason, start, success_return_to};

struct Db {
    driver: SsoDriver,
    repo: UserRepository,
    pool: DbPool,
    tag: String,
}

async fn db_harness() -> Db {
    let pool = test_db_pool().await;
    let users = Arc::new(UserService::new(Arc::new(UserRepository::new(&pool))));
    Db {
        driver: driver_on(
            &pool,
            "https://sso.test",
            users,
            Arc::new(CountingSessions::default()),
        ),
        repo: UserRepository::new(&pool),
        tag: uuid::Uuid::new_v4().simple().to_string(),
        pool,
    }
}

impl Db {
    fn email(&self) -> String {
        format!("owner-{}@example.test", self.tag)
    }

    fn sub(&self) -> String {
        format!("sub-{}", self.tag)
    }

    async fn owner(&self, roles: &[&str]) -> User {
        let user = self
            .repo
            .create(&format!("owner-{}", self.tag), &self.email(), None, None)
            .await
            .expect("create owner");
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        self.repo
            .assign_roles(&user.id, &roles)
            .await
            .expect("assign roles")
    }

    async fn sign_in(&self, provider: &str, groups: &str) -> FinishOutcome {
        let started = start(&self.driver, provider, None).await;
        let sub = self.sub();
        let email = self.email();
        let mut params = vec![
            ("state", started.state.as_str()),
            ("sub", sub.as_str()),
            ("email", email.as_str()),
            ("verified", "1"),
        ];
        if !groups.is_empty() {
            params.push(("groups", groups));
        }
        finish(&self.driver, provider, &params, Some(&started.binder)).await
    }

    async fn link_row(&self, issuer: &str) -> Option<(String, bool)> {
        sqlx::query_as::<_, (String, bool)>(
            "SELECT user_id, provisioned FROM federated_identities WHERE issuer = $1 AND \
             external_sub = $2",
        )
        .bind(issuer)
        .bind(self.sub())
        .fetch_optional(self.pool.pool().as_ref())
        .await
        .expect("link probe")
    }

    async fn roles_of(&self, id: &UserId) -> Vec<String> {
        self.repo
            .find_by_id(id)
            .await
            .expect("find")
            .expect("row")
            .roles
    }

    async fn cleanup(&self, id: &UserId) {
        let _ = self.repo.delete(id).await;
    }
}

#[tokio::test]
async fn an_email_clash_on_a_provider_without_linking_is_refused() {
    let h = db_harness().await;
    let owner = h.owner(&["user"]).await;

    let outcome = h.sign_in(UNLINKED, "admins").await;

    assert_eq!(refusal_reason(&outcome), "email_in_use");
    assert!(
        h.link_row("https://idp.test/unlinked").await.is_none(),
        "no link row is written"
    );
    assert_eq!(h.roles_of(&owner.id).await, vec!["user".to_owned()]);
    h.cleanup(&owner.id).await;
}

#[tokio::test]
async fn linking_an_ordinary_account_keeps_its_roles_across_sign_ins() {
    let h = db_harness().await;
    let owner = h.owner(&["user"]).await;

    success_return_to(&h.sign_in(OPEN, "admins").await);
    assert_eq!(
        h.link_row(OPEN_ISSUER).await,
        Some((owner.id.as_str().to_owned(), false))
    );
    assert_eq!(
        h.roles_of(&owner.id).await,
        vec!["user".to_owned()],
        "the directory's admin group is not written onto a pre-existing account"
    );

    success_return_to(&h.sign_in(OPEN, "admins").await);
    assert_eq!(h.roles_of(&owner.id).await, vec!["user".to_owned()]);
    h.cleanup(&owner.id).await;
}

#[tokio::test]
async fn admin_and_elevated_accounts_are_never_linked() {
    for roles in [&["admin", "user"][..], &["user", "billing"][..]] {
        let h = db_harness().await;
        let owner = h.owner(roles).await;

        let outcome = h.sign_in(OPEN, "").await;

        assert_eq!(refusal_reason(&outcome), "link_ineligible", "{roles:?}");
        assert!(h.link_row(OPEN_ISSUER).await.is_none());
        h.cleanup(&owner.id).await;
    }
}

#[tokio::test]
async fn a_suspended_account_is_never_linked() {
    let h = db_harness().await;
    let owner = h.owner(&["user"]).await;
    h.repo
        .update_status(&owner.id, UserStatus::Suspended)
        .await
        .expect("suspend");

    assert_eq!(
        refusal_reason(&h.sign_in(OPEN, "").await),
        "link_ineligible"
    );
    assert!(h.link_row(OPEN_ISSUER).await.is_none());
    h.cleanup(&owner.id).await;
}

#[tokio::test]
async fn a_service_account_is_never_linked() {
    let h = db_harness().await;
    let creator = h
        .repo
        .create(
            &format!("creator-{}", h.tag),
            &format!("creator-{}@example.test", h.tag),
            None,
            None,
        )
        .await
        .expect("creator");
    let service = h
        .repo
        .ensure_service_account(NewServiceAccount {
            name: format!("svc-{}", h.tag),
            email: h.email(),
            created_by: creator.id.clone(),
        })
        .await
        .expect("service account")
        .user;

    assert_eq!(
        refusal_reason(&h.sign_in(OPEN, "").await),
        "link_ineligible"
    );
    assert!(h.link_row(OPEN_ISSUER).await.is_none());
    h.cleanup(&service.id).await;
    h.cleanup(&creator.id).await;
}

#[tokio::test]
async fn a_provisioned_account_takes_the_directory_roles_and_follows_them() {
    let h = db_harness().await;

    success_return_to(&h.sign_in(OPEN, "admins").await);
    let (user_id, provisioned) = h.link_row(OPEN_ISSUER).await.expect("link row");
    assert!(provisioned);
    let user_id = UserId::new(user_id);
    assert_eq!(
        h.roles_of(&user_id).await,
        vec!["admin".to_owned(), RESOLVED_ROLE.to_owned()]
    );

    success_return_to(&h.sign_in(OPEN, "").await);
    assert_eq!(
        h.roles_of(&user_id).await,
        vec!["user".to_owned(), RESOLVED_ROLE.to_owned()],
        "a re-login of a provisioned account re-applies the directory's roles"
    );
    h.cleanup(&user_id).await;
}
