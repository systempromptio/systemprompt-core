#![allow(clippy::print_stdout)]

use std::time::Duration;
use systemprompt_core_logging::services::cli::{
    render_phase_warning, render_service_table, render_startup_banner, render_startup_complete,
    BrandColors, ServiceStatus, ServiceTableEntry,
};
use systemprompt_traits::{ServiceInfo, ServiceState};

pub struct StartupBanner;

impl StartupBanner {
    pub fn render(subtitle: Option<&str>) {
        render_startup_banner(subtitle);
    }
}

pub fn render_warning(message: &str) {
    render_phase_warning(message, None);
}

pub struct ServiceTable;

impl ServiceTable {
    pub fn render(title: &str, services: &[ServiceInfo]) {
        let entries: Vec<ServiceTableEntry> = services
            .iter()
            .map(|s| {
                let status = match s.state {
                    ServiceState::Running => ServiceStatus::Running,
                    ServiceState::Starting => ServiceStatus::Starting,
                    ServiceState::Stopped => ServiceStatus::Stopped,
                    ServiceState::Failed => ServiceStatus::Failed,
                };
                ServiceTableEntry::new(&s.name, s.service_type.label(), s.port, status)
            })
            .collect();

        render_service_table(title, &entries);
    }
}

pub struct CompletionMessage;

impl CompletionMessage {
    pub fn render_success(duration: Duration, api_url: &str) {
        render_startup_complete(duration, api_url);
    }

    pub fn render_failure(duration: Duration, error: &str) {
        println!();
        println!(
            "{} {} {}",
            BrandColors::stopped("✗"),
            BrandColors::white_bold("Startup failed"),
            BrandColors::dim(format!("after {:.1}s", duration.as_secs_f64()))
        );
        println!("  {}", BrandColors::stopped(error));
        println!();
    }
}
