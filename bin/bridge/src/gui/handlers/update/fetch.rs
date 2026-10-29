//! Gateway calls behind the update handlers: the authenticated client, the
//! version check and the staged install.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::error::GuiError;
use crate::gui::events::InstalledUpdate;
use crate::update::{self, UpdateUiState};

pub(super) async fn check(http: reqwest::Client) -> Result<UpdateUiState, GuiError> {
    let (client, bearer) = client_and_bearer(http).await?;
    let (status, _) = update::check_now(&client, &bearer).await?;
    Ok(UpdateUiState::from(&status))
}

pub(super) async fn install(
    version: &str,
    http: reqwest::Client,
    on_progress: &(dyn Fn(update::DownloadProgress) + Send + Sync),
) -> Result<InstalledUpdate, GuiError> {
    let (client, bearer) = client_and_bearer(http).await?;
    let (_, manifest) = update::check(&client, &bearer).await?;
    if manifest.version != version {
        return Err(update::UpdateError::VersionChanged {
            expected: version.to_owned(),
            actual: manifest.version,
        }
        .into());
    }
    let path = update::apply(&client, &bearer, &manifest, on_progress).await?;
    Ok(InstalledUpdate {
        version: manifest.version,
        path,
    })
}

pub(crate) async fn client_and_bearer(
    http: reqwest::Client,
) -> Result<(crate::gateway::GatewayClient, crate::ids::BearerToken), GuiError> {
    let cfg = crate::config::load()?;
    let gateway_url = crate::config::gateway_url_or_default(&cfg);
    let bearer = crate::auth::obtain_live_token(
        &cfg,
        &crate::auth::session_identity::stable_session_id(&cfg),
        &http,
    )
    .await
    .map(|out| out.token)
    .map_err(|e| GuiError::Profile {
        context: "update authentication".into(),
        source: std::io::Error::other(e),
    })?;
    Ok((
        crate::gateway::GatewayClient::new(gateway_url, http),
        bearer,
    ))
}
