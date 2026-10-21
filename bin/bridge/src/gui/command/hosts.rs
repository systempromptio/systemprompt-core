//! Host and agent command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::hosts::handlers::PersonalKeyOp;
use crate::wire::ipc::BridgeError;

use super::args::{
    HostEnabledArgs, HostIdArgs, HostInstallArgs, HostModelFilterArgs, PersonalKeyArgs,
};
use super::{CommandOutcome, parse, send};

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn host_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "host.probe" => host_probe(app, args, reply_id),
        "host.profile.generate" => host_profile_generate(app, args, reply_id),
        "host.profile.install" => host_profile_install(app, args, reply_id),
        "host.model-filter.set" => match parse::<HostModelFilterArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::Host(HostUiEvent::ModelFilterSetRequested {
                        host_id: a.host_id,
                        protocols: a.protocols,
                        reply_to: reply_id,
                    }),
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "host.enabled.set" => match parse::<HostEnabledArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::Host(HostUiEvent::EnabledSetRequested {
                        host_id: a.host_id,
                        enabled: a.enabled,
                        reply_to: reply_id,
                    }),
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "personalKeys.list" => personal_key(app, Ok(PersonalKeyOp::List), reply_id),
        "personalKeys.set" => personal_key(
            app,
            parse::<PersonalKeyArgs>(args).and_then(|a| match a.api_key {
                Some(api_key) => Ok(PersonalKeyOp::Set {
                    provider: a.provider,
                    api_key,
                    label: a.label,
                }),
                None => Err(BridgeError::invalid_args("apiKey is required")),
            }),
            reply_id,
        ),
        "personalKeys.remove" => personal_key(
            app,
            parse::<PersonalKeyArgs>(args).map(|a| PersonalKeyOp::Remove {
                provider: a.provider,
            }),
            reply_id,
        ),
        "host.proxy.probe" => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProxyProbeRequested { reply_to: reply_id }),
            );
            CommandOutcome::Async
        },
        _ => return None,
    })
}

fn personal_key(
    app: &GuiApp,
    op: Result<PersonalKeyOp, BridgeError>,
    reply_id: ReplyId,
) -> CommandOutcome {
    match op {
        Ok(op) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::PersonalKeyRequested {
                    op,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
fn host_probe(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostIdArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProbeRequested {
                    host_id: a.host_id,
                    cause: ProbeCause::Manual,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
fn host_profile_generate(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostIdArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProfileGenerateRequested {
                    host_id: a.host_id,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
fn host_profile_install(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostInstallArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProfileInstallRequested {
                    host_id: a.host_id,
                    path: a.path,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}
