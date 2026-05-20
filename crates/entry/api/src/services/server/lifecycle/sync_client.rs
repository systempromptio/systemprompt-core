use anyhow::Result;
use systemprompt_config::SecretsBootstrap;
use systemprompt_oauth::services::provision_sync_oauth_client;
use systemprompt_runtime::AppContext;

pub async fn provision_sync_client(ctx: &AppContext) -> Result<()> {
    let secrets = SecretsBootstrap::get()?;

    // Why: an unset SYNC_TOKEN means sync is disabled for this deployment; with
    // no `sys_sync` client provisioned, every `/api/v1/sync/*` request is
    // rejected by the authz framework.
    let Some(sync_token) = secrets.sync_token.as_deref() else {
        tracing::info!("SYNC_TOKEN not configured; sync OAuth client not provisioned");
        return Ok(());
    };

    provision_sync_oauth_client(ctx.db_pool(), sync_token).await?;
    tracing::info!("Provisioned sys_sync OAuth client");
    Ok(())
}
