//! The oauth member provides the plugin-token minter sealed under the
//! `identity-plugin-tokens` conferral row: only a command `identity-http`
//! owns with core standing reads it, and it mints the one hook-token shape for
//! an active admin over a fresh CLI session row, refusing anyone else.

use std::sync::Arc;

use base64::Engine;
use systemprompt_extension::reserved::{ConferralRow, CoreId};
use systemprompt_extension::{Resolved, StandingTag};
use systemprompt_identifiers::{ExtensionId, PluginId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row_with_roles, test_database_url,
};
use systemprompt_traits::{
    DynPluginTokenMinter, PluginTokenHolder, PluginTokenMintError, PluginTokenRequest,
};

const PLUGIN: &str = "conferred-plugin";

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn minter(ctx: &AppContext) -> DynPluginTokenMinter {
    let kernel = ctx.kernel().as_ref();
    let owner = ExtensionId::new(CoreId::IdentityHttp.id());
    Resolved::__member_command_owned(kernel, kernel.state(), &owner, StandingTag::Core)
        .get::<DynPluginTokenMinter>()
        .expect("an identity-http command reads the minter")
}

fn request(holder: PluginTokenHolder, duration_days: u32) -> PluginTokenRequest {
    PluginTokenRequest {
        holder,
        plugin_id: PluginId::new(PLUGIN),
        duration_days,
    }
}

async fn sessions_of(ctx: &AppContext, user: &UserId) -> Vec<String> {
    sqlx::query_scalar("SELECT session_id FROM user_sessions WHERE user_id = $1")
        .bind(user.as_str())
        .fetch_all(ctx.db_pool().pool().as_ref())
        .await
        .expect("read the backing sessions")
}

#[tokio::test]
async fn the_minter_is_conferred_on_identity_http_and_withheld_from_every_other_reader() {
    let ctx = context().await;
    let kernel = ctx.kernel().as_ref();
    let state = kernel.state();
    assert!(state.is_sealed::<DynPluginTokenMinter>());
    let conferred = state
        .conferral_of::<DynPluginTokenMinter>()
        .expect("the minter is conferred");
    assert_eq!(conferred.holder.as_str(), CoreId::IdentityHttp.id());
    assert_eq!(conferred.row, ConferralRow::IdentityPluginTokens.name());

    let reads = |owner: &str, standing| {
        let owner = ExtensionId::new(owner);
        Resolved::__member_command_owned(kernel, state, &owner, standing)
            .optional::<DynPluginTokenMinter>()
            .is_some()
    };
    assert!(reads("identity-http", StandingTag::Core));
    assert!(!reads("identity-http", StandingTag::Install));
    assert!(!reads("an-install", StandingTag::Install));
    assert!(!reads("gateway-http", StandingTag::Core));
    assert!(!reads("users", StandingTag::Core));
    assert!(
        Resolved::__member_command(kernel, state)
            .optional::<DynPluginTokenMinter>()
            .is_none(),
        "an install command reads no minter"
    );
    assert!(
        kernel
            .runtime_state()
            .find::<DynPluginTokenMinter>()
            .is_none()
    );
}

#[tokio::test]
async fn the_minter_issues_a_hook_token_for_an_admin_over_one_session_row_and_refuses_the_rest() {
    let ctx = context().await;
    let minter = minter(&ctx);
    let admin = UserId::new(uuid::Uuid::new_v4().to_string());
    let member = UserId::new(uuid::Uuid::new_v4().to_string());
    let admin_email = format!("plugin-admin-{}@example.invalid", admin.as_str());
    let member_email = format!("plugin-member-{}@example.invalid", member.as_str());
    seed_user_row_with_roles(ctx.db_pool(), &admin, &admin_email, &["admin".to_owned()])
        .await
        .expect("seed the admin");
    seed_user_row_with_roles(ctx.db_pool(), &member, &member_email, &["user".to_owned()])
        .await
        .expect("seed the member");

    let minted = minter
        .mint(request(PluginTokenHolder::Email(admin_email.clone()), 7))
        .await
        .expect("an admin is minted for");
    assert_eq!(minted.user_id, admin);
    assert_eq!(minted.email, admin_email);
    let claims = minted.token.split('.').nth(1).expect("a JWT");
    let claims: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(claims)
            .expect("decode the claims"),
    )
    .expect("claims JSON");
    assert_eq!(claims["aud"], serde_json::json!(["hook", "plugin"]));
    assert_eq!(claims["scope"], "hook:govern hook:track");
    assert_eq!(claims["plugin_id"], PLUGIN);
    assert_eq!(claims["sub"], admin.as_str());
    assert_eq!(claims["jti"], minted.jti.as_str());
    assert_eq!(
        claims["exp"].as_i64().expect("exp") - claims["iat"].as_i64().expect("iat"),
        7 * 24 * 60 * 60
    );
    let backing = sessions_of(&ctx, &admin).await;
    assert_eq!(backing.len(), 1);
    assert_eq!(claims["session_id"], backing[0]);

    let by_id = minter
        .mint(request(PluginTokenHolder::User(admin.clone()), 1))
        .await
        .expect("an admin named by id is minted for");
    assert_eq!(by_id.email, admin_email);

    assert!(matches!(
        minter
            .mint(request(PluginTokenHolder::Email(member_email), 7))
            .await,
        Err(PluginTokenMintError::NotAdmin { .. })
    ));
    assert!(sessions_of(&ctx, &member).await.is_empty());
    assert!(matches!(
        minter
            .mint(request(
                PluginTokenHolder::Email("nobody@example.invalid".to_owned()),
                7
            ))
            .await,
        Err(PluginTokenMintError::UserNotFound { .. })
    ));
    for days in [0, 366] {
        assert!(matches!(
            minter
                .mint(request(PluginTokenHolder::User(admin.clone()), days))
                .await,
            Err(PluginTokenMintError::Duration(d)) if d == days
        ));
    }
    assert_eq!(sessions_of(&ctx, &admin).await.len(), 2);
}
