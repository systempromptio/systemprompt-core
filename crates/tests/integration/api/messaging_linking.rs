//! The chat-platform path (`dispatch_messaging`) under the app's link policy:
//! a confirmed workspace email that an existing account owns is denied unless
//! the app links by email, and privileged or suspended accounts are never
//! claimed.

use systemprompt_api::routes::messaging::{DispatchOutcome, MessagingInbound, dispatch_messaging};
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_messaging_bootstrap, seed_agent_backend, test_app_context, test_db_pool,
};
use systemprompt_traits::{FederatedIdentityClaims, LinkPolicy, SenderIdentity};
use systemprompt_users::{User, UserStatus};
use uuid::Uuid;
use wiremock::MockServer;

use crate::messaging_dispatch::{ISSUER, inbound, mount_reply};

const LINK_ON: LinkPolicy = LinkPolicy {
    link_by_email: true,
    auto_provision: true,
};

struct Ctx {
    app: std::sync::Arc<AppContext>,
    pool: systemprompt_database::DbPool,
    tag: String,
    _backend: MockServer,
}

async fn setup() -> anyhow::Result<Ctx> {
    let b = ensure_messaging_bootstrap();
    let pool = test_db_pool().await;
    let app = test_app_context(&pool, &b.database_url);
    let backend = MockServer::start().await;
    mount_reply(&backend, "ok").await;
    seed_agent_backend(&app, &backend).await?;
    Ok(Ctx {
        app,
        pool,
        tag: Uuid::new_v4().simple().to_string(),
        _backend: backend,
    })
}

impl Ctx {
    fn email(&self) -> String {
        format!("member-{}@slack.invalid", self.tag)
    }

    fn sender(&self) -> String {
        format!("U_{}", self.tag)
    }

    fn message(&self, link: LinkPolicy) -> MessagingInbound {
        MessagingInbound {
            sender: SenderIdentity::Linked(FederatedIdentityClaims {
                email: Some(self.email()),
                email_verified: true,
                name: Some("Workspace Member".to_owned()),
                preferred_username: None,
                roles: Vec::new(),
            }),
            link,
            ..inbound(&self.sender(), "hello")
        }
    }

    async fn owner(&self, roles: &[&str]) -> anyhow::Result<User> {
        let repo = self.app.user_repository();
        let user = repo
            .create(&format!("member-{}", self.tag), &self.email(), None, None)
            .await?;
        sqlx::query("UPDATE users SET email_verified = true WHERE id = $1")
            .bind(user.id.as_str())
            .execute(self.pool.pool().as_ref())
            .await?;
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        Ok(repo.assign_roles(&user.id, &roles).await?)
    }

    async fn linked_user(&self) -> anyhow::Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "SELECT user_id FROM federated_identities WHERE issuer = $1 AND external_sub = $2",
        )
        .bind(ISSUER)
        .bind(self.sender())
        .fetch_optional(self.pool.pool().as_ref())
        .await?)
    }

    async fn roles_of(&self, id: &UserId) -> anyhow::Result<Vec<String>> {
        Ok(self
            .app
            .user_repository()
            .find_by_id(id)
            .await?
            .map(|user| user.roles)
            .unwrap_or_default())
    }

    async fn cleanup(&self, id: &UserId) {
        let _ = self.app.user_repository().delete(id).await;
    }
}

async fn denied(ctx: &Ctx, link: LinkPolicy) -> anyhow::Result<String> {
    match dispatch_messaging(&ctx.app, ctx.message(link))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?
    {
        DispatchOutcome::Denied(reason) => Ok(reason),
        DispatchOutcome::Replied(text) => panic!("expected a refusal, got a reply: {text}"),
    }
}

#[tokio::test]
async fn a_workspace_email_clash_without_linking_is_denied() -> anyhow::Result<()> {
    let ctx = setup().await?;
    let owner = ctx.owner(&["user"]).await?;

    let reason = denied(&ctx, LinkPolicy::default()).await?;

    assert!(reason.contains("linking is disabled"), "{reason}");
    assert!(ctx.linked_user().await?.is_none(), "no link row is written");
    assert_eq!(ctx.roles_of(&owner.id).await?, vec!["user".to_owned()]);
    ctx.cleanup(&owner.id).await;
    Ok(())
}

#[tokio::test]
async fn an_app_that_links_by_email_reaches_the_ordinary_owner() -> anyhow::Result<()> {
    let ctx = setup().await?;
    let owner = ctx.owner(&["user"]).await?;

    let outcome = dispatch_messaging(&ctx.app, ctx.message(LINK_ON))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;

    assert!(
        matches!(outcome, DispatchOutcome::Replied(_)),
        "{outcome:?}"
    );
    assert_eq!(ctx.linked_user().await?.as_deref(), Some(owner.id.as_str()));
    assert_eq!(ctx.roles_of(&owner.id).await?, vec!["user".to_owned()]);
    ctx.cleanup(&owner.id).await;
    Ok(())
}

#[tokio::test]
async fn an_admin_owner_is_never_claimed_by_a_workspace_sender() -> anyhow::Result<()> {
    let ctx = setup().await?;
    let owner = ctx.owner(&["admin", "user"]).await?;

    let reason = denied(&ctx, LINK_ON).await?;

    assert!(reason.contains("cannot be linked"), "{reason}");
    assert!(ctx.linked_user().await?.is_none());
    ctx.cleanup(&owner.id).await;
    Ok(())
}

#[tokio::test]
async fn a_suspended_owner_is_never_claimed_by_a_workspace_sender() -> anyhow::Result<()> {
    let ctx = setup().await?;
    let owner = ctx.owner(&["user"]).await?;
    ctx.app
        .user_repository()
        .update_status(&owner.id, UserStatus::Suspended)
        .await?;

    let reason = denied(&ctx, LINK_ON).await?;

    assert!(reason.contains("cannot be linked"), "{reason}");
    assert!(ctx.linked_user().await?.is_none());
    ctx.cleanup(&owner.id).await;
    Ok(())
}

async fn first_contact(ctx: &Ctx) -> anyhow::Result<UserId> {
    let outcome = dispatch_messaging(&ctx.app, ctx.message(LinkPolicy::default()))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;
    assert!(
        matches!(outcome, DispatchOutcome::Replied(_)),
        "{outcome:?}"
    );
    Ok(UserId::new(ctx.linked_user().await?.expect("link row")))
}

#[tokio::test]
async fn an_existing_link_to_a_suspended_account_is_denied_before_any_token() -> anyhow::Result<()>
{
    let ctx = setup().await?;
    let id = first_contact(&ctx).await?;
    ctx.app
        .user_repository()
        .update_status(&id, UserStatus::Suspended)
        .await?;

    let reason = denied(&ctx, LinkPolicy::default()).await?;

    assert!(reason.contains("not active"), "{reason}");
    ctx.cleanup(&id).await;
    Ok(())
}

#[tokio::test]
async fn an_existing_link_to_a_bot_account_is_denied_before_any_token() -> anyhow::Result<()> {
    let ctx = setup().await?;
    let id = first_contact(&ctx).await?;
    sqlx::query("UPDATE users SET is_bot = true WHERE id = $1")
        .bind(id.as_str())
        .execute(ctx.pool.pool().as_ref())
        .await?;

    let reason = denied(&ctx, LinkPolicy::default()).await?;

    assert!(reason.contains("not active"), "{reason}");
    ctx.cleanup(&id).await;
    Ok(())
}
