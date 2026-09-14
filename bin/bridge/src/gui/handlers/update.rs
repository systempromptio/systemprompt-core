//! GUI self-update handlers: check, install, restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use std::time::{Duration, Instant};
use winit::event_loop::ActiveEventLoop;


use crate::gui::error::GuiError;
use crate::gui::events::{InstalledUpdate, ReplyId, UiEvent};
use crate::gui::{GuiApp, emit};
use crate::update::{self, UpdateUiState};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use systemprompt_identifiers::SessionId;

const AUTO_UPDATE_INTERVAL: Duration = Duration::from_hours(6);

// Why: staging only — the check leads to a download and an on-disk swap, never
// a restart. Called from the one-second event-loop pass, so the policy read,
// which touches the last-sync sentinel, sits behind the interval test.
pub(crate) fn maybe_auto_check(app: &mut GuiApp, woke_from_sleep: bool) {
    if app.auto_update_pending {
        return;
    }
    let due = woke_from_sleep
        || app
            .last_update_check_at
            .is_none_or(|at| at.elapsed() >= AUTO_UPDATE_INTERVAL);
    if !due {
        return;
    }
    let snap = app.state.snapshot();
    if !snap.signed_in() || snap.update.in_progress() || snap.update.can_restart() {
        return;
    }
    if !update::auto_update_policy().stages() {
        return;
    }
    app.last_update_check_at = Some(Instant::now());
    app.auto_update_pending = true;
    on_update_check_requested(app, None);
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_update_check_requested(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = check(http).await.map_err(Arc::new);
        proxy.send_event(UiEvent::UpdateCheckFinished { result, reply_to });
    });
}

pub(crate) fn on_update_check_finished(
    app: &mut GuiApp,
    result: Result<UpdateUiState, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    match &result {
        Ok(state) => app.state.set_update_state(state.clone()),
        Err(e) => {
            tracing::debug!(error = %e, "update check failed");
        },
    }
    let staging = std::mem::take(&mut app.auto_update_pending)
        && app.state.snapshot().update.can_install()
        && update::auto_update_policy().stages();
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update check");
    if staging {
        app.append_log("a newer release is available; staging it for the next restart");
        app.proxy
            .send_event(UiEvent::UpdateInstallRequested { reply_to: None });
    }
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
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
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
        let result = install(&version, http, &on_progress)
            .await
            .map_err(Arc::new);
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
    result: Result<InstalledUpdate, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    match &result {
        Ok(installed) => {
            let version = installed.version.clone();
            crate::gui::window::notify_user(
                &format!(
                    "{} {version} is ready to install",
                    crate::brand::brand().app_name
                ),
                "Restart from the account menu to finish the update.",
            );
            app.state.set_update_state(UpdateUiState::Ready { version });
        },
        Err(e) => {
            let message = format!("{e}");
            app.append_log(format!("update failed: {message}"));
            app.state
                .set_update_state(UpdateUiState::Failed { message });
        },
    }
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update install");
}

pub(crate) fn on_update_restart_requested(app: &GuiApp, event_loop: &dyn ActiveEventLoop) {
    let installed = match update::installed_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "update: cannot resolve the installed path to relaunch");
            app.append_log(format!("restart failed: {e}"));
            return;
        },
    };
    // Why: the successor binds the default proxy port, so this instance stops
    // serving before it is spawned; host profiles written for 48217 are
    // rejected against any other port.
    let drained = app
        .ctx
        .proxy
        .served()
        .is_none_or(|served| served.drain(crate::proxy::DRAIN_DEADLINE));
    if !drained {
        app.append_log("restart: proxy requests were still in flight at the drain deadline");
    }
    if let Err(e) = update::spawn_successor(&installed) {
        tracing::error!(error = %e, "update: relaunch failed; leaving this instance running");
        app.append_log(format!(
            "restart failed: {e}; the update is installed — reopen manually"
        ));
        return;
    }
    crate::gui::handlers::quit::on_quit(event_loop);
}

async fn check(http: reqwest::Client) -> Result<UpdateUiState, GuiError> {
    let (client, bearer) = client_and_bearer(http).await?;
    let (status, _) = update::check(&client, &bearer).await?;
    Ok(UpdateUiState::from(&status))
}

async fn install(
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

async fn client_and_bearer(
    http: reqwest::Client,
) -> Result<(crate::gateway::GatewayClient, crate::ids::BearerToken), GuiError> {
    let cfg = crate::config::load()?;
    let gateway_url = crate::config::gateway_url_or_default(&cfg);
    let bearer = crate::auth::obtain_live_token(&cfg, &SessionId::generate(), &http)
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

fn reply<T: serde::Serialize>(
    app: &GuiApp,
    reply_to: ReplyId,
    result: Result<T, Arc<GuiError>>,
    what: &str,
) {
    if reply_to.is_none() {
        return;
    }
    let result = result.map_err(|err| {
        let raw = format!("{err:#}");
        tracing::warn!(error = %raw, operation = what, "update operation failed");
        BridgeError::new(ErrorScope::Internal, ErrorCode::Internal, raw)
    });
    emit::finish(app, reply_to, result);
}
