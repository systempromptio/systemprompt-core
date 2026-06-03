use systemprompt_identifiers::UserId;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::bridge::manifest::UserInfo;
use systemprompt_models::services::ServicesConfig;
use systemprompt_oauth::repository::BridgeHostPrefsRepository;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRepository;

pub async fn load_user(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Option<UserInfo>> {
    let repo = UserRepository::new(ctx.db_pool())?;
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
    let repo = UserRepository::new(ctx.db_pool())?;
    let ids = repo.list_revoked_api_key_ids_for_user(user_id).await?;
    Ok(ids)
}

pub async fn load_enabled_hosts(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Vec<String>> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    Ok(repo.list_enabled(user_id).await?)
}

pub async fn upsert_host_pref(
    ctx: &AppContext,
    user_id: &UserId,
    host_id: &str,
    enabled: bool,
) -> anyhow::Result<()> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    repo.upsert(user_id, host_id, enabled).await?;
    Ok(())
}

pub async fn load_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
) -> anyhow::Result<Vec<(String, Vec<String>)>> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    Ok(repo.load_model_protocols(user_id).await?)
}

pub async fn set_host_model_protocols(
    ctx: &AppContext,
    user_id: &UserId,
    host_id: &str,
    protocols: Option<&[String]>,
) -> anyhow::Result<()> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    repo.set_model_protocols(user_id, host_id, protocols)
        .await?;
    Ok(())
}

pub fn load_services_config() -> anyhow::Result<ServicesConfig> {
    ConfigLoader::load().map_err(|e| anyhow::anyhow!("services config load: {e}"))
}
