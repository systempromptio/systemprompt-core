#![allow(clippy::print_stdout)]

pub mod display;
pub mod module;
pub mod prompts;
pub mod summary;
pub mod theme;

pub use display::{
    render_phase_header, render_phase_info, render_phase_success, render_phase_warning,
    render_service_table, render_startup_banner, render_startup_complete, CollectionDisplay,
    Display, DisplayUtils, ModuleItemDisplay, ServiceTableEntry, StatusDisplay,
};
pub use module::{BatchModuleOperations, ModuleDisplay, ModuleInstall, ModuleUpdate};
pub use prompts::{PromptBuilder, Prompts, QuickPrompts};
pub use summary::{OperationResult, ProgressSummary, ValidationSummary};
pub use theme::{
    ActionType, BrandColors, Colors, EmphasisType, IconType, Icons, ItemStatus, MessageLevel,
    ModuleType, ServiceStatus, Theme,
};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::time::Duration;
use systemprompt_traits::LogEventLevel;

use super::output::{is_console_output_enabled, publish_log};

#[derive(Copy, Clone, Debug)]
pub struct CliService;

impl CliService {
    pub fn success(message: &str) {
        publish_log(LogEventLevel::Info, "cli", message);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Success, message);
        }
    }

    pub fn warning(message: &str) {
        publish_log(LogEventLevel::Warn, "cli", message);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Warning, message);
        }
    }

    pub fn error(message: &str) {
        publish_log(LogEventLevel::Error, "cli", message);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Error, message);
        }
    }

    pub fn info(message: &str) {
        publish_log(LogEventLevel::Info, "cli", message);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Info, message);
        }
    }

    pub fn debug(message: &str) {
        let debug_msg = format!("DEBUG: {message}");
        publish_log(LogEventLevel::Debug, "cli", &debug_msg);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Info, &debug_msg);
        }
    }

    pub fn verbose(message: &str) {
        publish_log(LogEventLevel::Debug, "cli", message);
        if is_console_output_enabled() {
            DisplayUtils::message(MessageLevel::Info, message);
        }
    }

    #[allow(clippy::exit)] // Intentional for CLI fatal errors
    pub fn fatal(message: &str, exit_code: i32) -> ! {
        let fatal_msg = format!("FATAL: {message}");
        DisplayUtils::message(MessageLevel::Error, &fatal_msg);
        std::process::exit(exit_code);
    }

    pub fn section(title: &str) {
        DisplayUtils::section_header(title);
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
        display::render_table(headers, rows);
    }

    pub fn startup_banner(subtitle: Option<&str>) {
        if is_console_output_enabled() {
            render_startup_banner(subtitle);
        }
    }

    pub fn phase(name: &str) {
        publish_log(LogEventLevel::Info, "cli", &format!("Phase: {}", name));
        if is_console_output_enabled() {
            render_phase_header(name);
        }
    }

    pub fn phase_success(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Info, "cli", message);
        if is_console_output_enabled() {
            render_phase_success(message, detail);
        }
    }

    pub fn phase_info(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Info, "cli", message);
        if is_console_output_enabled() {
            render_phase_info(message, detail);
        }
    }

    pub fn phase_warning(message: &str, detail: Option<&str>) {
        publish_log(LogEventLevel::Warn, "cli", message);
        if is_console_output_enabled() {
            render_phase_warning(message, detail);
        }
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
        if is_console_output_enabled() {
            render_service_table(title, services);
        }
    }

    pub fn startup_complete(duration: Duration, api_url: &str) {
        publish_log(
            LogEventLevel::Info,
            "cli",
            &format!("Startup complete in {:.1}s", duration.as_secs_f64()),
        );
        if is_console_output_enabled() {
            render_startup_complete(duration, api_url);
        }
    }
}

#[macro_export]
macro_rules! cli_success {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::success(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_warning {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::warning(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_error {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::error(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_info {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::info(&format!($($arg)*))
    };
}
