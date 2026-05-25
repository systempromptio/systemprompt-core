//! Banner and session-context rendering helpers for [`CliService`].
//!
//! These methods own the side-effect of writing branded headers and footers to
//! stderr. They are co-located with [`CliService`] but split out for cohesion.

use std::io::Write;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use systemprompt_traits::LogEventLevel;

use super::output::publish_log;
use super::service::CliService;
use super::startup::{
    render_phase_header, render_phase_info, render_phase_success, render_phase_warning,
    render_startup_banner,
};
use super::table::{ServiceTableEntry, render_service_table, render_startup_complete};
use super::theme::{EmphasisType, Theme};

impl CliService {
    pub fn startup_banner(subtitle: Option<&str>) {
        render_startup_banner(subtitle);
    }

    pub fn phase(name: &str) {
        publish_log(LogEventLevel::Info, "cli", &format!("Phase: {}", name));
        render_phase_header(name);
    }

    pub fn phase_success(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Info, "cli", message);
        render_phase_success(message, detail);
    }

    pub fn phase_info(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Info, "cli", message);
        render_phase_info(message, detail);
    }

    pub fn phase_warning(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Warn, "cli", message);
        render_phase_warning(message, detail);
    }

    pub fn service_spinner(service_name: &str, port: Option<u16>) -> ProgressBar {
        let msg = port.map_or_else(
            || format!("Starting {}", service_name),
            |p| format!("Starting {} on :{}", service_name, p),
        );
        let pb = ProgressBar::new_spinner();
        let spinner_template = concat!("{spinner:.208}", " {msg}");
        pb.set_style(
            ProgressStyle::default_spinner()
                .template(spinner_template)
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(msg);
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }

    pub fn service_table(title: &str, services: &[ServiceTableEntry]) {
        render_service_table(title, services);
    }

    pub fn startup_complete(duration: Duration, api_url: &str) {
        publish_log(
            LogEventLevel::Info,
            "cli",
            &format!("Startup complete in {:.1}s", duration.as_secs_f64()),
        );
        render_startup_complete(duration, api_url);
    }

    pub fn session_context(
        profile: &str,
        session_id: &systemprompt_identifiers::SessionId,
        tenant: Option<&str>,
    ) {
        Self::session_context_with_url(profile, session_id, tenant, None);
    }

    pub fn session_context_with_url(
        profile: &str,
        session_id: &systemprompt_identifiers::SessionId,
        tenant: Option<&str>,
        api_url: Option<&str>,
    ) {
        let session_str = session_id.as_str();
        let truncated_session = session_str
            .get(..12)
            .map_or_else(|| session_str.to_owned(), |s| format!("{}...", s));

        let tenant_info = tenant.map_or_else(String::new, |t| format!(" | tenant: {}", t));

        let url_info = api_url.map_or_else(String::new, |u| format!(" | {}", u));

        let banner = format!(
            "[profile: {} | session: {}{}{}]",
            profile, truncated_session, tenant_info, url_info
        );

        let mut stderr = std::io::stderr();
        writeln!(stderr, "{}", Theme::color(&banner, EmphasisType::Dim)).ok();
    }

    pub fn profile_banner(profile_name: &str, is_cloud: bool, tenant: Option<&str>) {
        let target_label = if is_cloud { "cloud" } else { "local" };
        let tenant_info = tenant.map_or_else(String::new, |t| format!(" | tenant: {}", t));
        let banner = format!(
            "[profile: {} ({}){}]",
            profile_name, target_label, tenant_info
        );
        let mut stderr = std::io::stderr();
        writeln!(stderr, "{}", Theme::color(&banner, EmphasisType::Dim)).ok();
    }
}
