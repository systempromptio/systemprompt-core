//! Bridge data endpoints backing manifest sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::bridge::manifest::UserInfo;
use systemprompt_models::services::ServicesConfig;
use systemprompt_runtime::AppContext;

pub async fn load_user(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Option<UserInfo>> {
    let repo = ctx.user_repository();
    let Some(user) = repo.find_by_id(user_id).await? else {
        return Ok(None);
    };
    Ok(Some(UserInfo {
        id: user.id,
        name: user.name,
        email: user.email,
        display_name: user.display_name,
        roles: user.roles,
    }))
}

pub async fn load_revocations(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Vec<String>> {
    let repo = ctx.user_repository();
    let ids = repo.list_revoked_api_key_ids_for_user(user_id).await?;
    Ok(ids)
}

pub async fn load_enabled_hosts(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Vec<String>> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    Ok(repo.list_enabled(user_id).await?)
}

pub async fn upsert_host_pref(
    ctx: &AppContext,
    user_id: &UserId,
    host_id: &str,
    enabled: bool,
) -> anyhow::Result<()> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.upsert(user_id, host_id, enabled).await?;
    Ok(())
}

pub async fn load_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
) -> anyhow::Result<Vec<(String, Vec<String>)>> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    Ok(repo.load_model_protocols(user_id).await?)
}

pub async fn set_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
    host_id: &str,
    protocols: Option<&[String]>,
) -> anyhow::Result<()> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.set_model_protocols(user_id, host_id, protocols)
        .await?;
    Ok(())
}

pub fn load_services_config() -> anyhow::Result<ServicesConfig> {
    ConfigLoader::load().map_err(|e| anyhow::anyhow!("services config load: {e}"))
}
