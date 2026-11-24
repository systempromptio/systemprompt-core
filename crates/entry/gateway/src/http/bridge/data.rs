//! Bridge data endpoints backing manifest sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_models::bridge::manifest::UserInfo;
use systemprompt_oauth::OauthResult;
use systemprompt_oauth::repository::{EnabledHostPrefs, HostPrefWrite};
use systemprompt_users::UserError;

use super::BridgeRouteState;

pub async fn load_user(
    state: &BridgeRouteState,
    user_id: &UserId,
) -> Result<Option<UserInfo>, UserError> {
    let repo = &state.repos.users;
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
    state: &BridgeRouteState,
    user_id: &UserId,
) -> Result<Vec<ApiKeyId>, UserError> {
    let repo = &state.repos.users;
    let ids = repo.list_revoked_api_key_ids_for_user(user_id).await?;
    Ok(ids.into_iter().map(ApiKeyId::new).collect())
}

pub async fn load_enabled_hosts(
    state: &BridgeRouteState,
    user_id: &UserId,
) -> OauthResult<EnabledHostPrefs> {
    let repo = &state.repos.host_prefs;
    repo.get_enabled_prefs(user_id).await
}

pub async fn set_host_enabled(
    state: &BridgeRouteState,
    user_id: &UserId,
    host: HostKind,
    enabled: bool,
    instance: &InstanceHosts,
) -> OauthResult<HostPrefWrite> {
    let repo = &state.repos.host_prefs;
    repo.set_enabled(user_id, host, enabled, instance).await
}

pub async fn load_host_model_protocols(
    state: &BridgeRouteState,
    user_id: &UserId,
) -> OauthResult<Vec<(HostKind, Vec<String>)>> {
    let repo = &state.repos.host_prefs;
    repo.load_model_protocols(user_id).await
}

pub async fn set_host_model_protocols(
    state: &BridgeRouteState,
    user_id: &UserId,
    host: HostKind,
    protocols: Option<&[String]>,
) -> OauthResult<()> {
    let repo = &state.repos.host_prefs;
    repo.set_model_protocols(user_id, host, protocols).await?;
    Ok(())
}
