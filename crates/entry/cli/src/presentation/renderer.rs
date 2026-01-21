use systemprompt_traits::{
    Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent, StartupEventReceiver,
};

use super::state::RenderState;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use systemprompt_logging::services::cli::BrandColors;

use super::widgets::{render_warning, CompletionMessage, ServiceTable, StartupBanner};

pub struct StartupRenderer {
    receiver: StartupEventReceiver,
    state: RenderState,
}

impl StartupRenderer {
    pub fn new(receiver: StartupEventReceiver) -> Self {
        Self {
            receiver,
            state: RenderState::new(),
        }
    }

    pub async fn run(mut self) {
        StartupBanner::render(Some("Starting services..."));

        while let Some(event) = self.receiver.recv().await {
            if self.handle_event(event) {
                break;
            }
        }
    }

    fn handle_event(&mut self, event: StartupEvent) -> bool {
        match event {
            StartupEvent::PhaseStarted { phase } => {
                self.state.finish_all_spinners();
                self.state.current_phase = Some(phase);
                self.state.is_blocking = phase.is_blocking();

                if matches!(phase, Phase::McpServers | Phase::Agents) {
                    let spinner = Self::create_phase_spinner(phase.name());
                    self.state
                        .spinners
                        .insert(format!("phase_{}", phase.name()), spinner);
                }
            },

            StartupEvent::PhaseCompleted { phase } => {
                let phase_key = format!("phase_{}", phase.name());
                if let Some(spinner) = self.state.spinners.remove(&phase_key) {
                    spinner.finish_and_clear();
                    let (running, total) = match phase {
                        Phase::McpServers => self.state.mcp_count,
                        Phase::Agents => self.state.agent_count,
                        _ => (0, 0),
                    };
                    systemprompt_logging::CliService::info(&format!(
                        "  {} {} ({}/{})",
                        BrandColors::running("✓"),
                        phase.name(),
                        running,
                        total
                    ));
                }
            },

            StartupEvent::PhaseFailed { phase, error } => {
                let phase_key = format!("phase_{}", phase.name());
                if let Some(spinner) = self.state.spinners.remove(&phase_key) {
                    spinner.finish_and_clear();
                    systemprompt_logging::CliService::info(&format!(
                        "  {} {} failed: {}",
                        BrandColors::stopped("✗"),
                        phase.name(),
                        error
                    ));
                } else {
                    render_warning(&format!("{} failed: {}", phase.name(), error));
                }
            },

            StartupEvent::PortConflict { port, pid } => {
                render_warning(&format!("Port {} in use by PID {}", port, pid));
            },

            StartupEvent::PortCheckStarted { .. }
            | StartupEvent::PortAvailable { .. }
            | StartupEvent::PortConflictResolved { .. }
            | StartupEvent::ModulesLoaded { .. }
            | StartupEvent::MigrationStarted
            | StartupEvent::MigrationApplied { .. }
            | StartupEvent::MigrationComplete { .. }
            | StartupEvent::DatabaseValidated
            | StartupEvent::McpServerStarting { .. }
            | StartupEvent::McpServerHealthCheck { .. }
            | StartupEvent::McpServiceCleanup { .. }
            | StartupEvent::AgentStarting { .. }
            | StartupEvent::AgentCleanup { .. }
            | StartupEvent::RoutesConfiguring
            | StartupEvent::RoutesConfigured { .. }
            | StartupEvent::ExtensionRouteMounted { .. }
            | StartupEvent::ServerBinding { .. }
            | StartupEvent::ServerListening { .. }
            | StartupEvent::SchedulerJobRegistered { .. }
            | StartupEvent::BootstrapJobStarted { .. }
            | StartupEvent::BootstrapJobCompleted { .. }
            | StartupEvent::Info { .. } => {},

            StartupEvent::McpServerReady {
                name,
                port,
                startup_time,
                tools: _,
            } => {
                self.state.add_service(ServiceInfo {
                    name,
                    service_type: ServiceType::Mcp,
                    port: Some(port),
                    state: ServiceState::Running,
                    startup_time: Some(startup_time),
                });
            },

            StartupEvent::McpServerFailed { name, error } => {
                render_warning(&format!("MCP {} failed: {}", name, error));
                self.state.add_service(ServiceInfo {
                    name,
                    service_type: ServiceType::Mcp,
                    port: None,
                    state: ServiceState::Failed,
                    startup_time: None,
                });
            },

            StartupEvent::McpReconciliationComplete { running, required } => {
                self.state.mcp_count = (running, required);
            },

            StartupEvent::AgentReady {
                name,
                port,
                startup_time,
            } => {
                self.state.add_service(ServiceInfo {
                    name,
                    service_type: ServiceType::Agent,
                    port: Some(port),
                    state: ServiceState::Running,
                    startup_time: Some(startup_time),
                });
            },

            StartupEvent::AgentFailed { name, error } => {
                render_warning(&format!("Agent {} failed: {}", name, error));
                self.state.add_service(ServiceInfo {
                    name,
                    service_type: ServiceType::Agent,
                    port: None,
                    state: ServiceState::Failed,
                    startup_time: None,
                });
            },

            StartupEvent::AgentReconciliationComplete { running, total } => {
                self.state.agent_count = (running, total);
            },

            StartupEvent::SchedulerInitializing => {
                let spinner = Self::create_phase_spinner("Scheduler");
                self.state.spinners.insert("scheduler".to_string(), spinner);
            },

            StartupEvent::SchedulerReady { job_count } => {
                if let Some(spinner) = self.state.spinners.remove("scheduler") {
                    spinner.finish_and_clear();
                    systemprompt_logging::CliService::info(&format!(
                        "  {} Scheduler ({} jobs)",
                        BrandColors::running("✓"),
                        job_count
                    ));
                }
            },

            StartupEvent::Warning { message, context } => {
                self.state.warnings.push(message.clone());
                match context {
                    Some(ctx) => render_warning(&format!("{}: {}", message, ctx)),
                    None => render_warning(&message),
                }
            },

            StartupEvent::Error { message, fatal } => {
                if fatal {
                    self.state.finish_all_spinners();
                }
                render_warning(&format!("ERROR: {}", message));
            },

            StartupEvent::StartupComplete {
                duration,
                api_url,
                services,
            } => {
                self.state.finish_all_spinners();

                for svc in services {
                    if !self.state.services.iter().any(|s| s.name == svc.name) {
                        self.state.services.push(svc);
                    }
                }

                if !self.state.services.is_empty() {
                    ServiceTable::render("Services", &self.state.services);
                }

                CompletionMessage::render_success(duration, &api_url);
                return true;
            },

            StartupEvent::StartupFailed { error, duration } => {
                self.state.finish_all_spinners();
                CompletionMessage::render_failure(duration, &error);
                return true;
            },
        }

        false
    }

    fn create_phase_spinner(name: &str) -> ProgressBar {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        spinner.set_message(format!("{}...", name));
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner
    }
}
