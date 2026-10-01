//! Bridge data endpoints backing manifest sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_loader::{ConfigLoadResult, ConfigLoader};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::UserInfo;
use systemprompt_models::services::ServicesConfig;
use systemprompt_oauth::OauthResult;
use systemprompt_oauth::repository::EnabledHostPrefs;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserError;

pub async fn load_user(ctx: &AppContext, user_id: &UserId) -> Result<Option<UserInfo>, UserError> {
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

pub async fn load_revocations(
    ctx: &AppContext,
    user_id: &UserId,
) -> Result<Vec<ApiKeyId>, UserError> {
    let repo = ctx.user_repository();
    let ids = repo.list_revoked_api_key_ids_for_user(user_id).await?;
    Ok(ids.into_iter().map(ApiKeyId::new).collect())
}

pub async fn load_enabled_hosts(
    ctx: &AppContext,
    user_id: &UserId,
) -> OauthResult<EnabledHostPrefs> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.list_enabled(user_id).await
}

pub async fn upsert_host_pref(
    ctx: &AppContext,
    user_id: &UserId,
    host: HostKind,
    enabled: bool,
) -> OauthResult<()> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.upsert(user_id, host, enabled).await?;
    Ok(())
}

pub async fn load_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
) -> OauthResult<Vec<(HostKind, Vec<String>)>> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.load_model_protocols(user_id).await
}

pub async fn set_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
    host: HostKind,
    protocols: Option<&[String]>,
) -> OauthResult<()> {
    let repo = &ctx.oauth_repositories().bridge_host_prefs;
    repo.set_model_protocols(user_id, host, protocols).await?;
    Ok(())
}

pub fn load_services_config() -> ConfigLoadResult<ServicesConfig> {
    ConfigLoader::load()
}
