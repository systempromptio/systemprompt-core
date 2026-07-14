//! The [`CliService`] facade over CLI output.
//!
//! Aggregates the display, table, and progress helpers into a single entry
//! point for command code: levelled messages (which also publish a log event),
//! structured output (`json`/`yaml`), and spinners and progress bars. Output is
//! the sanctioned stderr/stdout sink, not `tracing`.

use std::io::Write;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use systemprompt_traits::LogEventLevel;

use super::display::DisplayUtils;
use super::output::{mark_structured_emitted, publish_log};
use super::theme::{EmphasisType, ItemStatus, MessageLevel, Theme};

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
        let mut stderr = std::io::stderr();
        if let Err(e) = write!(stderr, "\x1B[2J\x1B[1;1H") {
            tracing::debug!(error = %e, "cli clear_screen write failed");
        }
    }

    pub fn output(content: &str) {
        let mut stdout = std::io::stdout();
        if let Err(e) = writeln!(stdout, "{content}") {
            tracing::debug!(error = %e, "cli output write failed");
        }
    }

    pub fn json<T: Serialize>(value: &T) {
        mark_structured_emitted();
        match serde_json::to_string_pretty(value) {
            Ok(json) => {
                let mut stdout = std::io::stdout();
                if let Err(e) = writeln!(stdout, "{json}") {
                    tracing::debug!(error = %e, "cli json write failed");
                }
            },
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn json_compact<T: Serialize>(value: &T) {
        mark_structured_emitted();
        match serde_json::to_string(value) {
            Ok(json) => {
                let mut stdout = std::io::stdout();
                if let Err(e) = writeln!(stdout, "{json}") {
                    tracing::debug!(error = %e, "cli json_compact write failed");
                }
            },
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn yaml<T: Serialize>(value: &T) {
        mark_structured_emitted();
        match serde_yaml::to_string(value) {
            Ok(yaml) => {
                let mut stdout = std::io::stdout();
                if let Err(e) = write!(stdout, "{yaml}") {
                    tracing::debug!(error = %e, "cli yaml write failed");
                }
            },
            Err(e) => Self::error(&format!("Failed to format log entry: {e}")),
        }
    }

    pub fn key_value(label: &str, value: &str) {
        let mut stderr = std::io::stderr();
        if let Err(e) = writeln!(
            stderr,
            "{}: {}",
            Theme::color(label, EmphasisType::Bold),
            Theme::color(value, EmphasisType::Highlight)
        ) {
            tracing::debug!(error = %e, "cli key_value write failed");
        }
    }

    pub fn status_line(label: &str, value: &str, status: ItemStatus) {
        let mut stderr = std::io::stderr();
        if let Err(e) = writeln!(
            stderr,
            "{} {}: {}",
            Theme::icon(status),
            Theme::color(label, EmphasisType::Bold),
            Theme::color(value, status)
        ) {
            tracing::debug!(error = %e, "cli status_line write failed");
        }
    }

    pub fn spinner(message: &str) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(message.to_owned());
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

    pub fn table(headers: &[&str], rows: &[Vec<String>]) {
        super::table::render_table(headers, rows);
    }
}
