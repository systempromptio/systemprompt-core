//! Host and agent command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};

use super::args::{HostIdArgs, HostInstallArgs, HostModelFilterArgs};
use super::{CommandOutcome, parse, send};

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

pub(super) fn agent_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "agent.uninstall" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentUninstall {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.openConfig" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpenConfig {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.open" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpen {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        _ => return None,
    })
}

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
