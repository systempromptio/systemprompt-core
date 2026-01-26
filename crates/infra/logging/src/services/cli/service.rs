#![allow(clippy::print_stdout)]

use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use systemprompt_traits::LogEventLevel;

use super::display::{CollectionDisplay, Display, DisplayUtils};
use super::module::{BatchModuleOperations, ModuleDisplay, ModuleInstall, ModuleUpdate};
use super::output::publish_log;
use super::prompts::{PromptBuilder, Prompts};
use super::startup::{
    render_phase_header, render_phase_info, render_phase_success, render_phase_warning,
    render_startup_banner,
};
use super::summary::{OperationResult, ProgressSummary, ValidationSummary};
use super::table::{render_service_table, render_startup_complete, ServiceTableEntry};
use super::theme::{EmphasisType, ItemStatus, MessageLevel, ModuleType, Theme};

#[derive(Copy, Clone, Debug)]
pub struct CliService;

impl CliService {
    pub fn success(message: &str) {
        publish_log(LogEventLevel::Info, "cli", message);
        DisplayUtils::message(MessageLevel::Success, message);
    }

    pub fn warning(message: &str) {
        publish_log(LogEventLevel::Warn, "cli", message);
        DisplayUtils::message(MessageLevel::Warning, message);
    }

    pub fn error(message: &str) {
        publish_log(LogEventLevel::Error, "cli", message);
        DisplayUtils::message(MessageLevel::Error, message);
    }

    pub fn info(message: &str) {
        publish_log(LogEventLevel::Info, "cli", message);
        DisplayUtils::message(MessageLevel::Info, message);
    }

    pub fn debug(message: &str) {
        let debug_msg = format!("DEBUG: {message}");
        publish_log(LogEventLevel::Debug, "cli", &debug_msg);
        DisplayUtils::message(MessageLevel::Info, &debug_msg);
    }

    pub fn verbose(message: &str) {
        publish_log(LogEventLevel::Debug, "cli", message);
        DisplayUtils::message(MessageLevel::Info, message);
    }

    #[allow(clippy::exit)]
    pub fn fatal(message: &str, exit_code: i32) -> ! {
        let fatal_msg = format!("FATAL: {message}");
        DisplayUtils::message(MessageLevel::Error, &fatal_msg);
        std::process::exit(exit_code);
    }

    pub fn section(title: &str) {
        DisplayUtils::section_header(title);
    }

    pub fn subsection(title: &str) {
        DisplayUtils::subsection_header(title);
    }

    pub fn clear_screen() {
        print!("\x1B[2J\x1B[1;1H");
    }

    pub fn output(content: &str) {
        println!("{content}");
    }

    pub fn json<T: Serialize>(value: &T) {
        match serde_json::to_string_pretty(value) {
            Ok(json) => println!("{json}"),
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn json_compact<T: Serialize>(value: &T) {
        match serde_json::to_string(value) {
            Ok(json) => println!("{json}"),
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn yaml<T: Serialize>(value: &T) {
        match serde_yaml::to_string(value) {
            Ok(yaml) => print!("{yaml}"),
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn key_value(label: &str, value: &str) {
        println!(
            "{}: {}",
            Theme::color(label, EmphasisType::Bold),
            Theme::color(value, EmphasisType::Highlight)
        );
    }

    pub fn status_line(label: &str, value: &str, status: ItemStatus) {
        println!(
            "{} {}: {}",
            Theme::icon(status),
            Theme::color(label, EmphasisType::Bold),
            Theme::color(value, status)
        );
    }

    pub fn spinner(message: &str) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    }

    pub fn progress_bar(total: u64) -> ProgressBar {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("#>-"),
        );
        pb
    }

    pub fn timed<F, R>(label: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        let duration_secs = duration.as_secs_f64();
        let info_msg = format!("{label} completed in {duration_secs:.2}s");
        Self::info(&info_msg);
        result
    }

    pub fn prompt_schemas(module_name: &str, schemas: &[(String, String)]) -> Result<bool> {
        ModuleDisplay::prompt_apply_schemas(module_name, schemas)
    }

    pub fn prompt_seeds(module_name: &str, seeds: &[(String, String)]) -> Result<bool> {
        ModuleDisplay::prompt_apply_seeds(module_name, seeds)
    }

    pub fn prompt_install(modules: &[String]) -> Result<bool> {
        Prompts::confirm_install(modules)
    }

    pub fn prompt_update(updates: &[(String, String, String)]) -> Result<bool> {
        Prompts::confirm_update(updates)
    }

    pub fn confirm(question: &str) -> Result<bool> {
        Prompts::confirm(question, false)
    }

    pub fn confirm_default_yes(question: &str) -> Result<bool> {
        Prompts::confirm(question, true)
    }

    pub fn display_validation_summary(summary: &ValidationSummary) {
        summary.display();
    }

    pub fn display_result(result: &OperationResult) {
        result.display();
    }

    pub fn display_progress(progress: &ProgressSummary) {
        progress.display();
    }

    pub fn prompt_builder(message: &str) -> PromptBuilder {
        PromptBuilder::new(message)
    }

    pub fn collection<T: Display>(title: &str, items: Vec<T>) -> CollectionDisplay<T> {
        CollectionDisplay::new(title, items)
    }

    pub fn module_status(module_name: &str, message: &str) {
        DisplayUtils::module_status(module_name, message);
    }

    pub fn relationship(from: &str, to: &str, status: ItemStatus, module_type: ModuleType) {
        DisplayUtils::relationship(module_type, from, to, status);
    }

    pub fn item(status: ItemStatus, name: &str, detail: Option<&str>) {
        DisplayUtils::item(status, name, detail);
    }

    pub fn batch_install(modules: &[ModuleInstall]) -> Result<bool> {
        BatchModuleOperations::prompt_install_multiple(modules)
    }

    pub fn batch_update(updates: &[ModuleUpdate]) -> Result<bool> {
        BatchModuleOperations::prompt_update_multiple(updates)
    }

    pub fn table(headers: &[&str], rows: &[Vec<String>]) {
        super::table::render_table(headers, rows);
    }

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

    #[allow(clippy::literal_string_with_formatting_args)]
    pub fn service_spinner(service_name: &str, port: Option<u16>) -> ProgressBar {
        let msg = port.map_or_else(
            || format!("Starting {}", service_name),
            |p| format!("Starting {} on :{}", service_name, p),
        );
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.208} {msg}")
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
            .map_or_else(|| session_str.to_string(), |s| format!("{}...", s));

        let tenant_info = tenant.map_or_else(String::new, |t| format!(" | tenant: {}", t));

        let url_info = api_url.map_or_else(String::new, |u| format!(" | {}", u));

        let banner = format!(
            "[profile: {} | session: {}{}{}]",
            profile, truncated_session, tenant_info, url_info
        );

        println!("{}", Theme::color(&banner, EmphasisType::Dim));
    }

    pub fn profile_banner(profile_name: &str, is_cloud: bool, tenant: Option<&str>) {
        let target_label = if is_cloud { "cloud" } else { "local" };
        let tenant_info = tenant.map_or_else(String::new, |t| format!(" | tenant: {}", t));
        let banner = format!("[profile: {} ({}){}]", profile_name, target_label, tenant_info);
        eprintln!("{}", Theme::color(&banner, EmphasisType::Dim));
    }
}
