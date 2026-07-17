//! Mutable render state consumed by the startup renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use indicatif::ProgressBar;
use std::collections::HashMap;
use systemprompt_traits::{Phase, ServiceInfo, ServiceState, ServiceType};

#[derive(Debug)]
pub(super) struct RenderState {
    pub current_phase: Option<Phase>,
    pub spinners: HashMap<String, ProgressBar>,
    pub services: Vec<ServiceInfo>,
    pub warnings: Vec<String>,
    pub is_blocking: bool,
    pub mcp_count: (usize, usize),
    pub agent_count: (usize, usize),
}

impl RenderState {
    pub(super) fn new() -> Self {
        Self {
            current_phase: None,
            spinners: HashMap::new(),
            services: Vec::new(),
            warnings: Vec::new(),
            is_blocking: true,
            mcp_count: (0, 0),
            agent_count: (0, 0),
        }
    }

    pub(super) fn add_service(&mut self, info: ServiceInfo) {
        match info.service_type {
            ServiceType::Mcp => {
                if matches!(info.state, ServiceState::Running) {
                    self.mcp_count.0 += 1;
                }
            },
            ServiceType::Agent => {
                if matches!(info.state, ServiceState::Running) {
                    self.agent_count.0 += 1;
                }
            },
            _ => {},
        }
        self.services.push(info);
    }

    pub(super) fn finish_all_spinners(&mut self) {
        for (_, spinner) in self.spinners.drain() {
            spinner.finish_and_clear();
        }
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self::new()
    }
}
