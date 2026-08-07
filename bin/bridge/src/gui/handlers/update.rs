//! GUI self-update handlers: check, install, restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, emit};
use crate::update::{self, UpdateUiState};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_update_check_requested(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = check().await.map_err(Arc::new);
        proxy.send_event(UiEvent::UpdateCheckFinished { result, reply_to });
    });
}

pub(crate) fn on_update_check_finished(
    app: &mut GuiApp,
    result: Result<Value, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    match &result {
        Ok(value) => {
            let state = serde_json::from_value::<UpdateUiState>(value.clone())
                .unwrap_or(UpdateUiState::Unknown);
            app.state.set_update_state(state);
        },
        Err(e) => {
            // Why: the network is allowed to be down, so a failed check stays a
            // log line and leaves the button as it was rather than raising a toast.
            tracing::debug!(error = %e, "update check failed");
        },
    }
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update check");
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_update_install_requested(app: &mut GuiApp, reply_to: ReplyId) {
    let Some(version) = app.state.snapshot().update.version().map(str::to_owned) else {
        if let Some(id) = reply_to {
            let payload = IpcReplyPayload::err(BridgeError::new(
                ErrorScope::Internal,
                ErrorCode::Internal,
                "no update is pending".to_owned(),
            ));
            emit::send_reply_payload(app, id, &payload);
        }
        return;
    };

    app.state.set_update_state(UpdateUiState::Downloading {
        version: version.clone(),
        percent: 0,
    });
    app.refresh_ui();
    emit::emit_state(app);

    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let progress_proxy = proxy.clone();
        let progress_version = version.clone();
        let on_progress = move |p: update::DownloadProgress| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "fraction() is clamped to 0.0..=1.0, so the product fits u8"
            )]
            let percent = (p.fraction() * 100.0) as u8;
            progress_proxy.send_event(UiEvent::UpdateProgress {
                version: progress_version.clone(),
                percent,
            });
        };
        let result = install(&version, &on_progress).await.map_err(Arc::new);
        proxy.send_event(UiEvent::UpdateInstallFinished { result, reply_to });
    });
}

pub(crate) fn on_update_progress(app: &mut GuiApp, version: &str, percent: u8) {
    app.state.set_update_progress(version, percent);
    app.refresh_ui();
    emit::emit_state(app);
}

pub(crate) fn on_update_install_finished(
    app: &mut GuiApp,
    result: Result<Value, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    match &result {
        Ok(value) => {
            let version = value
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            app.state.set_update_state(UpdateUiState::Ready { version });
        },
        Err(e) => {
            let message = format!("{e}");
            app.append_log(format!("update failed: {message}"));
            app.state.set_update_state(UpdateUiState::Failed { message });
        },
    }
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update install");
}

// Why: the new process must start before this one exits — on Windows the
// displaced `.old` binary cannot be swept until this process releases its image
// lock, and the sweep runs at the new process's startup.
pub(crate) fn on_update_restart_requested(app: &GuiApp) {
    let installed = match update::installed_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "update: cannot resolve the installed path to relaunch");
            app.append_log(format!("restart failed: {e}"));
            return;
        },
    };
    if let Err(e) = update::spawn_installed(&installed) {
        tracing::error!(error = %e, "update: relaunch failed; leaving this instance running");
        app.append_log(format!("restart failed: {e}; the update is installed — reopen manually"));
        return;
    }
    crate::gui::handlers::quit::on_quit();
}

async fn check() -> Result<Value, GuiError> {
    let (client, bearer) = client_and_bearer()?;
    let (status, _) = update::check(&client, &bearer).await?;
    Ok(serde_json::to_value(UpdateUiState::from(&status)).unwrap_or(Value::Null))
}

async fn install(
    version: &str,
    on_progress: &(dyn Fn(update::DownloadProgress) + Send + Sync),
) -> Result<Value, GuiError> {
    let (client, bearer) = client_and_bearer()?;
    // Why: re-checked rather than trusting the version the button was rendered
    // with — the manifest carries the digest, and a release published in between
    // would otherwise be installed without the user having agreed to it.
    let (_, manifest) = update::check(&client, &bearer).await?;
    if manifest.version != version {
        return Err(update::UpdateError::VersionChanged {
            expected: version.to_owned(),
            actual: manifest.version,
        }
        .into());
    }
    let path = update::apply(&client, &bearer, &manifest, on_progress).await?;
    Ok(json!({ "version": manifest.version, "path": path.display().to_string() }))
}

fn client_and_bearer() -> Result<(crate::gateway::GatewayClient, String), GuiError> {
    let cfg = crate::config::load();
    let gateway_url = crate::config::gateway_url_or_default(&cfg);
    let bearer = crate::auth::cache::read_valid()
        .map(|out| out.token.expose().to_owned())
        .ok_or(GuiError::NotAuthenticated)?;
    Ok((crate::gateway::GatewayClient::new(gateway_url), bearer))
}

fn reply(app: &GuiApp, reply_to: ReplyId, result: Result<Value, Arc<GuiError>>, what: &str) {
    let Some(id) = reply_to else {
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            let raw = format!("{err:#}");
            tracing::warn!(error = %raw, "{what} failed");
            IpcReplyPayload::err(BridgeError::new(
                ErrorScope::Internal,
                ErrorCode::Internal,
                raw,
            ))
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
