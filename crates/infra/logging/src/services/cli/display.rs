//! Display primitives for CLI output.
//!
//! Defines the [`Display`] trait and the [`DisplayUtils`] helpers (levelled
//! messages, section headers). All output goes to stderr via this sanctioned
//! display sink.

use std::io::Write;

use crate::services::cli::theme::{EmphasisType, MessageLevel, Theme};

pub trait Display {
    fn display(&self);
}

fn stderr_writeln(args: std::fmt::Arguments<'_>) {
    let mut stderr = std::io::stderr();
    writeln!(stderr, "{args}").ok();
}

const fn message_level_str(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Success => "success",
        MessageLevel::Warning => "warning",
        MessageLevel::Error => "error",
        MessageLevel::Info => "info",
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DisplayUtils;

impl DisplayUtils {
    pub fn message(level: MessageLevel, text: &str) {
        if crate::services::output::is_structured_output() {
            crate::services::output::buffer_notice(message_level_str(level), text);
            return;
        }
        stderr_writeln(format_args!(
            "{} {}",
            Theme::icon(level),
            Theme::color(text, level)
        ));
    }

    pub fn section_header(title: &str) {
        stderr_writeln(format_args!(
            "\n{}",
            Theme::color(title, EmphasisType::Underlined)
        ));
    }

    pub fn subsection_header(title: &str) {
        stderr_writeln(format_args!(
            "\n  {}",
            Theme::color(title, EmphasisType::Bold)
        ));
    }
}
