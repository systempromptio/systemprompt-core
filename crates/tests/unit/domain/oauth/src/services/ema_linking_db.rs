//! The token-exchange path (`link_enterprise_principal`) against the real
//! users repository: the trusted issuer's link policy decides whether an
//! ID-JAG's verified email may claim an existing account, and privileged or
//! suspended accounts are never claimed.

use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::error::OauthError;
use systemprompt_oauth::services::EnterprisePrincipal;
use systemprompt_oauth::services::ema::link_enterprise_principal;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::LinkPolicy;
use systemprompt_users::{NewServiceAccount, User, UserRepository, UserService, UserStatus};

use crate::state::NullAnalytics;

const LINK_ON: LinkPolicy = LinkPolicy {
    link_by_email: true,
    auto_provision: true,
};

struct Ctx {
    state: OAuthState,
    repo: UserRepository,
    pool: systemprompt_database::DbPool,
    tag: String,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let users = Arc::new(UserService::new(Arc::new(UserRepository::new(&pool))));
    Ctx {
        state: OAuthState::new(
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
            Arc::new(NullAnalytics),
            Arc::new(NullAnalytics),
            users,
            systemprompt_test_fixtures::test_oauth_signing(),
        ),
        repo: UserRepository::new(&pool),
        pool,
        tag: uuid::Uuid::new_v4().simple().to_string(),
    }
}

impl Ctx {
    fn email(&self) -> String {
        format!("employee-{}@ema.invalid", self.tag)
    }

    fn issuer(&self) -> String {
        format!("https://idp-{}.invalid", self.tag)
    }

    fn principal(&self, link: LinkPolicy) -> EnterprisePrincipal {
        EnterprisePrincipal {
            issuer: self.issuer(),
            sub: format!("sub-{}", self.tag),
            email: Some(self.email()),
            email_verified: true,
            link,
        }
    }

    async fn owner(&self, roles: &[&str]) -> User {
        let user = self
            .repo
            .create(&format!("employee-{}", self.tag), &self.email(), None, None)
            .await
            .expect("create owner");
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        self.repo
            .assign_roles(&user.id, &roles)
            .await
            .expect("assign roles")
    }

    async fn link_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM federated_identities WHERE issuer = $1")
            .bind(self.issuer())
            .fetch_one(self.pool.pool().as_ref())
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

fn assert_refused(result: Result<impl std::fmt::Debug, OauthError>, expected: &str) {
    match result {
        Err(OauthError::InvalidGrant(reason)) => {
            assert!(reason.contains(expected), "unexpected reason: {reason}");
        },
        other => panic!("expected an invalid_grant refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn an_email_clash_under_the_default_issuer_policy_is_refused() {
    let ctx = setup().await;
    let owner = ctx.owner(&["user"]).await;

    let result = link_enterprise_principal(&ctx.state, &ctx.principal(LinkPolicy::default())).await;

    assert_refused(result, "linking is disabled");
    assert_eq!(
        ctx.link_count().await,
        0,
        "no link row and no second account"
    );
    assert_eq!(ctx.roles_of(&owner.id).await, vec!["user".to_owned()]);
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_issuer_that_links_by_email_claims_an_ordinary_account_without_touching_roles() {
    let ctx = setup().await;
    let owner = ctx.owner(&["user"]).await;

    let linked = link_enterprise_principal(&ctx.state, &ctx.principal(LINK_ON))
        .await
        .expect("linked");

    assert_eq!(linked.user_id, owner.id);
    assert_eq!(ctx.link_count().await, 1);
    assert_eq!(ctx.roles_of(&owner.id).await, vec!["user".to_owned()]);

    let again = link_enterprise_principal(&ctx.state, &ctx.principal(LINK_ON))
        .await
        .expect("re-use");
    assert_eq!(again.user_id, owner.id);
    assert_eq!(ctx.roles_of(&owner.id).await, vec!["user".to_owned()]);
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_admin_account_is_never_claimed_by_an_id_jag() {
    let ctx = setup().await;
    let owner = ctx.owner(&["admin", "user"]).await;

    let result = link_enterprise_principal(&ctx.state, &ctx.principal(LINK_ON)).await;

    assert_refused(result, "cannot be linked");
    assert_eq!(ctx.link_count().await, 0);
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn a_suspended_account_is_never_claimed_by_an_id_jag() {
    let ctx = setup().await;
    let owner = ctx.owner(&["user"]).await;
    ctx.repo
        .update_status(&owner.id, UserStatus::Suspended)
        .await
        .expect("suspend");

    let result = link_enterprise_principal(&ctx.state, &ctx.principal(LINK_ON)).await;

    assert_refused(result, "cannot be linked");
    assert_eq!(ctx.link_count().await, 0);
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn a_service_account_is_never_claimed_by_an_id_jag() {
    let ctx = setup().await;
    let creator = ctx
        .repo
        .create(
            &format!("creator-{}", ctx.tag),
            &format!("creator-{}@ema.invalid", ctx.tag),
            None,
            None,
        )
        .await
        .expect("creator");
    let service = ctx
        .repo
        .ensure_service_account(NewServiceAccount {
            name: format!("svc-{}", ctx.tag),
            email: ctx.email(),
            created_by: creator.id.clone(),
        })
        .await
        .expect("service account")
        .user;

    let result = link_enterprise_principal(&ctx.state, &ctx.principal(LINK_ON)).await;

    assert_refused(result, "cannot be linked");
    assert_eq!(ctx.link_count().await, 0);
    ctx.cleanup(&service.id).await;
    ctx.cleanup(&creator.id).await;
}

#[tokio::test]
async fn a_new_enterprise_identity_is_provisioned_as_an_ordinary_user() {
    let ctx = setup().await;

    let linked = link_enterprise_principal(&ctx.state, &ctx.principal(LinkPolicy::default()))
        .await
        .expect("provisioned");

    assert_eq!(linked.email, ctx.email());
    assert_eq!(ctx.link_count().await, 1);
    assert_eq!(ctx.roles_of(&linked.user_id).await, vec!["user".to_owned()]);
    ctx.cleanup(&linked.user_id).await;
}
