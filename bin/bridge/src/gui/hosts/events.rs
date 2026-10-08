//! `HostUiEvent` definitions for host-app status changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_models::bridge::host::HostKind;

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::gui::hosts::state::ProbeSeq;
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCause {
    Tick,
    Manual,
}

#[derive(Debug, Clone)]
pub enum HostUiEvent {
    ProbeFailed {
        host_id: Option<(HostKind, ProbeSeq)>,
        error: String,
        reply_to: ReplyId,
    },
    ProbeRequested {
        host_id: HostKind,
        cause: ProbeCause,
        reply_to: ReplyId,
    },
    ProbeFinished {
        host_id: HostKind,
        seq: ProbeSeq,
        cause: ProbeCause,
        snapshot: Box<HostAppSnapshot>,
        reply_to: ReplyId,
    },
    ProfileGenerateRequested {
        host_id: HostKind,
        reply_to: ReplyId,
    },
    ProfileGenerateFinished {
        host_id: HostKind,
        result: Result<GeneratedProfile, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    ProfileInstallRequested {
        host_id: HostKind,
        path: String,
        reply_to: ReplyId,
    },
    ProfileInstallFinished {
        host_id: HostKind,
        result: Result<(String, Vec<String>), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    ProxyProbeRequested {
        reply_to: ReplyId,
    },
    ProxyProbeFinished {
        health: Box<ProxyHealth>,
        reply_to: ReplyId,
    },
    ModelFilterSetRequested {
        host_id: HostKind,
        protocols: Option<Vec<String>>,
        reply_to: ReplyId,
    },
    ModelFilterSetFinished {
        host_id: HostKind,
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    UnattendedRepairFinished {
        host_id: HostKind,
        report: crate::integration::reapply::Report,
    },
}
