use std::sync::Arc;

use crate::gui::error::GuiError;
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};

#[derive(Debug, Clone)]
pub enum HostUiEvent {
    ProbeRequested {
        host_id: String,
    },
    ProbeFinished {
        host_id: String,
        snapshot: Box<HostAppSnapshot>,
    },
    ProfileGenerateRequested {
        host_id: String,
    },
    ProfileGenerateFinished {
        host_id: String,
        result: Result<GeneratedProfile, Arc<GuiError>>,
    },
    ProfileInstallRequested {
        host_id: String,
        path: String,
    },
    ProfileInstallFinished {
        host_id: String,
        result: Result<String, Arc<GuiError>>,
    },
    ProxyProbeRequested,
    ProxyProbeFinished(Box<ProxyHealth>),
}
