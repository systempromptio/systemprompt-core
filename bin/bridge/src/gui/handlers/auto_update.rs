//! The scheduled self-update trigger. It only raises [`Trigger::Schedule`] /
//! [`Trigger::Wake`]; what an unattended update is allowed to do lives in
//! [`crate::update::automatic`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::update::{client_and_bearer, progress_reporter};
use crate::gui::error::GuiError;
use crate::gui::events::{InstalledUpdate, UiEvent};
use crate::gui::{GuiApp, UiEventProxy};
use crate::update::UpdateUiState;
use crate::update::automatic::{Permit, Trigger};

// Why: called from the one-second event-loop pass; `UpdateGate::begin` owns the
// due time, org policy and single flight, so this only raises the trigger.
pub(crate) fn maybe_auto_check(app: &GuiApp, woke_from_sleep: bool) {
    let snap = app.state.snapshot();
    if !snap.signed_in() || snap.update.in_progress() || snap.update.can_restart() {
        return;
    }
    let trigger = if woke_from_sleep {
        Trigger::Wake
    } else {
        Trigger::Schedule
    };
    let Ok(permit) = app.ctx.update_gate.begin(trigger) else {
        return;
    };
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(stage_automatically(permit, http, proxy));
}

async fn stage_automatically(permit: Permit, http: reqwest::Client, proxy: UiEventProxy) {
    let checked = async {
        let (client, bearer) = client_and_bearer(http).await?;
        let manifest = permit.check(&client, &bearer).await?;
        Ok::<_, GuiError>((client, bearer, manifest))
    }
    .await;
    let (client, bearer, manifest) = match checked {
        Ok((client, bearer, Some(manifest))) => (client, bearer, manifest),
        Ok((_, _, None)) => {
            proxy.send_event(UiEvent::UpdateCheckFinished {
                result: Ok(UpdateUiState::Current),
                reply_to: None,
                automatic: true,
            });
            return;
        },
        Err(e) => {
            proxy.send_event(UiEvent::UpdateCheckFinished {
                result: Err(Arc::new(e)),
                reply_to: None,
                automatic: true,
            });
            return;
        },
    };
    proxy.send_event(UiEvent::UpdateCheckFinished {
        result: Ok(UpdateUiState::Available {
            version: manifest.version.clone(),
            notes_url: manifest.notes_url.clone(),
        }),
        reply_to: None,
        automatic: true,
    });
    let on_progress = progress_reporter(proxy.clone(), manifest.version.clone());
    let result = permit
        .stage(&client, &bearer, &manifest, &on_progress)
        .await
        .map(|path| InstalledUpdate {
            version: manifest.version,
            path,
        })
        .map_err(|e| Arc::new(GuiError::from(e)));
    proxy.send_event(UiEvent::UpdateInstallFinished {
        result,
        reply_to: None,
    });
}
