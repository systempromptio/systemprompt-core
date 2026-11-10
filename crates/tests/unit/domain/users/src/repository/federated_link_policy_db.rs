//! DB-backed tests for the link policy `resolve_federated` enforces: an email
//! clash refuses unless the issuer links by email, only ordinary accounts are
//! ever linked, and a link records whether the issuer provisioned its account.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::{FederatedIdentityClaims, FederatedLink, FederatedRefusal, LinkPolicy};
use systemprompt_users::{NewServiceAccount, User, UserError, UserRepository, UserStatus};
use uuid::Uuid;

const LINK_ON: LinkPolicy = LinkPolicy {
    link_by_email: true,
    auto_provision: true,
};
const LINK_OFF: LinkPolicy = LinkPolicy {
    link_by_email: false,
    auto_provision: true,
};

struct Ctx {
    repo: UserRepository,
    pool: systemprompt_database::DbPool,
    issuer: String,
    sub: String,
    tag: String,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let tag = Uuid::new_v4().simple().to_string();
    Ctx {
        repo: UserRepository::new(&pool),
        pool,
        issuer: format!("https://idp-{tag}.invalid"),
        sub: format!("sub-{tag}"),
        tag,
    }
}

impl Ctx {
    fn email(&self) -> String {
        format!("owner-{}@link.invalid", self.tag)
    }

    async fn ordinary(&self) -> User {
        self.repo
            .create(&format!("owner-{}", self.tag), &self.email(), None, None)
            .await
            .expect("create owner")
    }

    async fn with_roles(&self, roles: &[&str]) -> User {
        let user = self.ordinary().await;
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        self.repo
            .assign_roles(&user.id, &roles)
            .await
            .expect("assign roles")
    }

    fn claims(&self, roles: &[&str]) -> FederatedIdentityClaims {
        FederatedIdentityClaims {
            email: Some(self.email().to_uppercase()),
            email_verified: true,
            name: Some("Federated Person".to_owned()),
            preferred_username: None,
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        }
    }

    async fn resolve(
        &self,
        roles: &[&str],
        policy: &LinkPolicy,
    ) -> Result<(User, FederatedLink), UserError> {
        self.repo
            .resolve_federated(&self.issuer, &self.sub, &self.claims(roles), policy)
            .await
    }

    async fn link_row(&self) -> Option<(String, bool)> {
        sqlx::query_as::<_, (String, bool)>(
            "SELECT user_id, provisioned FROM federated_identities WHERE issuer = $1 AND \
             external_sub = $2",
        )
        .bind(&self.issuer)
        .bind(&self.sub)
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

fn refusal(result: Result<(User, FederatedLink), UserError>) -> FederatedRefusal {
    match result {
        Err(UserError::FederatedRefused(refusal)) => refusal,
        other => panic!("expected a federated refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn an_email_clash_with_linking_off_is_refused_without_a_link_or_second_account() {
    let ctx = setup().await;
    let owner = ctx.ordinary().await;

    let result = ctx.resolve(&["admin"], &LINK_OFF).await;

    assert_eq!(refusal(result), FederatedRefusal::EmailInUse);
    assert!(ctx.link_row().await.is_none(), "no link row is written");
    assert_eq!(ctx.roles_of(&owner.id).await, vec!["user".to_owned()]);
    let federated_for_issuer: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM federated_identities WHERE issuer = $1")
            .bind(&ctx.issuer)
            .fetch_one(ctx.pool.pool().as_ref())
            .await
            .expect("issuer probe");
    assert_eq!(
        federated_for_issuer, 0,
        "no account is provisioned for the issuer"
    );
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn linking_on_links_an_ordinary_account_and_leaves_its_roles_alone() {
    let ctx = setup().await;
    let owner = ctx.ordinary().await;

    let (user, link) = ctx.resolve(&["admin"], &LINK_ON).await.expect("linked");

    assert_eq!(user.id, owner.id);
    assert_eq!(link, FederatedLink::Linked);
    assert!(!link.issuer_owns_roles());
    assert_eq!(
        ctx.link_row().await,
        Some((owner.id.as_str().to_owned(), false))
    );
    assert_eq!(ctx.roles_of(&owner.id).await, vec!["user".to_owned()]);

    let (again, relink) = ctx.resolve(&["admin"], &LINK_ON).await.expect("re-login");
    assert_eq!(again.id, owner.id);
    assert_eq!(relink, FederatedLink::Existing { provisioned: false });
    assert!(!relink.issuer_owns_roles());
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_admin_account_is_never_linked() {
    let ctx = setup().await;
    let owner = ctx.with_roles(&["admin", "user"]).await;

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_account_holding_any_role_beyond_user_is_never_linked() {
    let ctx = setup().await;
    let owner = ctx.with_roles(&["user", "billing"]).await;

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn a_suspended_account_is_never_linked() {
    let ctx = setup().await;
    let owner = ctx.ordinary().await;
    ctx.repo
        .update_status(&owner.id, UserStatus::Suspended)
        .await
        .expect("suspend");

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn a_service_account_is_never_linked() {
    let ctx = setup().await;
    let creator = ctx
        .repo
        .create(
            &format!("creator-{}", ctx.tag),
            &format!("creator-{}@link.invalid", ctx.tag),
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

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&service.id).await;
    ctx.cleanup(&creator.id).await;
}

#[tokio::test]
async fn a_new_identity_is_provisioned_with_the_issuer_roles_and_marked_provisioned() {
    let ctx = setup().await;

    let (user, link) = ctx
        .resolve(&["operator"], &LINK_OFF)
        .await
        .expect("provisioned");

    assert_eq!(link, FederatedLink::Provisioned);
    assert!(link.issuer_owns_roles());
    assert_eq!(user.roles, vec!["operator".to_owned()]);
    assert_eq!(
        ctx.link_row().await,
        Some((user.id.as_str().to_owned(), true))
    );

    let (_, relink) = ctx
        .resolve(&["operator"], &LINK_OFF)
        .await
        .expect("re-login");
    assert_eq!(relink, FederatedLink::Existing { provisioned: true });
    assert!(relink.issuer_owns_roles());
    ctx.cleanup(&user.id).await;
}

#[tokio::test]
async fn provisioning_off_refuses_a_new_identity() {
    let ctx = setup().await;
    let policy = LinkPolicy {
        link_by_email: false,
        auto_provision: false,
    };

    assert_eq!(
        refusal(ctx.resolve(&[], &policy).await),
        FederatedRefusal::NotProvisioned
    );
    assert!(ctx.link_row().await.is_none());
}

#[test]
fn the_default_policy_never_links_by_email() {
    assert_eq!(LinkPolicy::default(), LINK_OFF);
    let parsed: LinkPolicy = serde_json::from_str("{}").expect("empty policy decodes");
    assert_eq!(parsed, LINK_OFF);
}
