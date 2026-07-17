//! `HostUiEvent` definitions for host-app status changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCause {
    Tick,
    Manual,
}

#[derive(Debug, Clone)]
pub enum HostUiEvent {
    ProbeRequested {
        host_id: String,
        cause: ProbeCause,
        reply_to: ReplyId,
    },
    ProbeFinished {
        host_id: String,
        cause: ProbeCause,
        snapshot: Box<HostAppSnapshot>,
        reply_to: ReplyId,
    },
    ProfileGenerateRequested {
        host_id: String,
        reply_to: ReplyId,
    },
    ProfileGenerateFinished {
        host_id: String,
        result: Result<GeneratedProfile, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    ProfileInstallRequested {
        host_id: String,
        path: String,
        reply_to: ReplyId,
    },
    ProfileInstallFinished {
        host_id: String,
        result: Result<String, Arc<GuiError>>,
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
        host_id: String,
        /// `None` clears the override; `Some([])` means "all models".
        protocols: Option<Vec<String>>,
        reply_to: ReplyId,
    },
    ModelFilterSetFinished {
        host_id: String,
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
}
