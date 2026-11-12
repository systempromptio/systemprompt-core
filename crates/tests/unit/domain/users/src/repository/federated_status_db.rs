//! `resolve_federated` owns the sign-in status rule and the email-link
//! preconditions: an existing link to an account that is not active or not
//! an interactive human account is refused, an email link needs a verified
//! email on both sides, and an email held by a deleted account refuses rather
//! than colliding.

use systemprompt_identifiers::UserId;
use systemprompt_traits::{FederatedIdentityClaims, FederatedLink, FederatedRefusal};
use systemprompt_users::{NewServiceAccount, UserStatus};

use super::federated_link_policy_db::{Ctx, LINK_OFF, LINK_ON, refusal, setup};

impl Ctx {
    async fn provisioned(&self) -> UserId {
        let (user, link) = self.resolve(&[], &LINK_OFF).await.expect("provisioned");
        assert_eq!(link, FederatedLink::Provisioned);
        user.id
    }

    async fn sql(&self, statement: &'static str, id: &UserId) {
        sqlx::query(statement)
            .bind(id.as_str())
            .execute(self.pool.pool().as_ref())
            .await
            .expect("fixture statement");
    }
}

#[tokio::test]
async fn an_existing_link_to_a_suspended_account_is_refused() {
    let ctx = setup().await;
    let id = ctx.provisioned().await;
    ctx.repo
        .update_status(&id, UserStatus::Suspended)
        .await
        .expect("suspend");

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_OFF).await),
        FederatedRefusal::AccountUnavailable
    );
    ctx.cleanup(&id).await;
}

#[tokio::test]
async fn an_existing_link_to_a_bot_account_is_refused() {
    let ctx = setup().await;
    let id = ctx.provisioned().await;
    ctx.sql("UPDATE users SET is_bot = true WHERE id = $1", &id)
        .await;

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_OFF).await),
        FederatedRefusal::AccountUnavailable
    );
    ctx.cleanup(&id).await;
}

#[tokio::test]
async fn an_existing_link_to_a_service_account_is_refused() {
    let ctx = setup().await;
    let creator = ctx.ordinary().await;
    let service = ctx
        .repo
        .ensure_service_account(NewServiceAccount {
            name: format!("svc-{}", ctx.tag),
            email: format!("svc-{}@link.invalid", ctx.tag),
            created_by: creator.id.clone(),
        })
        .await
        .expect("service account")
        .user;
    sqlx::query(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
    )
    .bind(&ctx.issuer)
    .bind(&ctx.sub)
    .bind(service.id.as_str())
    .execute(ctx.pool.pool().as_ref())
    .await
    .expect("seed link");

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_OFF).await),
        FederatedRefusal::AccountUnavailable
    );
    ctx.cleanup(&service.id).await;
    ctx.cleanup(&creator.id).await;
}

#[tokio::test]
async fn a_target_whose_own_email_is_unverified_is_never_linked() {
    let ctx = setup().await;
    let owner = ctx.unverified().await;

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_unverified_incoming_email_never_links() {
    let ctx = setup().await;
    let owner = ctx.ordinary().await;
    let claims = FederatedIdentityClaims {
        email_verified: false,
        ..ctx.claims(&[])
    };

    let (user, link) = ctx
        .repo
        .resolve_federated(&ctx.issuer, &ctx.sub, &claims, &LINK_ON)
        .await
        .expect("separate account");

    assert_eq!(link, FederatedLink::Provisioned);
    assert_ne!(user.id, owner.id);
    assert!(user.email.ends_with(".federated.local"), "{}", user.email);
    ctx.cleanup(&user.id).await;
    ctx.cleanup(&owner.id).await;
}

#[tokio::test]
async fn an_email_held_by_a_deleted_account_is_a_typed_refusal() {
    let ctx = setup().await;
    let owner = ctx.ordinary().await;
    ctx.repo
        .update_status(&owner.id, UserStatus::Deleted)
        .await
        .expect("delete");

    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_OFF).await),
        FederatedRefusal::EmailInUse
    );
    assert_eq!(
        refusal(ctx.resolve(&[], &LINK_ON).await),
        FederatedRefusal::TargetIneligible
    );
    assert!(ctx.link_row().await.is_none());
    ctx.cleanup(&owner.id).await;
}
