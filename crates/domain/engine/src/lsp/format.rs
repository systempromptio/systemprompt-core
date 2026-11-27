//! Diagnostics as compact text for the model.
//!
//! One block per file, files with errors first, each line `  <severity>
//! <line>:<column> <message> [<code>] (<source>)` with one-based positions,
//! capped per file and in number of files. The text is empty when nothing is at
//! or above the minimum severity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;
use crate::types::{Diagnostic, Severity};

/// Limits and presentation of [`format_diagnostics`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticFormat {
    pub base: Option<PathBuf>,
    pub min_severity: Severity,
    pub max_files: usize,
    pub max_per_file: usize,
    pub max_message_chars: usize,
}

impl Default for DiagnosticFormat {
    fn default() -> Self {
        Self {
            base: None,
            min_severity: Severity::Warning,
            max_files: 10,
            max_per_file: 20,
            max_message_chars: 400,
        }
    }
}

pub fn format_diagnostics(diagnostics: &[Diagnostic], options: &DiagnosticFormat) -> String {
    let mut by_file: BTreeMap<&Path, Vec<&Diagnostic>> = BTreeMap::new();
    for diagnostic in diagnostics
        .iter()
        .filter(|d| d.severity <= options.min_severity)
    {
        by_file
            .entry(diagnostic.path.as_path())
            .or_default()
            .push(diagnostic);
    }
    if by_file.is_empty() {
        return String::new();
    }

    let mut files: Vec<(&Path, Vec<&Diagnostic>)> = by_file.into_iter().collect();
    for (_, items) in &mut files {
        items.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then(a.range.start.cmp(&b.range.start))
        });
        items.dedup();
    }
    files.sort_by_key(|(path, items)| (std::cmp::Reverse(error_count(items)), *path));

    let mut lines = Vec::new();
    let shown_files = files.len().min(options.max_files.max(1));
    for (path, items) in files.iter().take(shown_files) {
        lines.push(paths::display_relative(path, options.base.as_deref()));
        for diagnostic in items.iter().take(options.max_per_file.max(1)) {
            lines.push(line(diagnostic, options.max_message_chars));
        }
        let hidden = items.len().saturating_sub(options.max_per_file.max(1));
        if hidden > 0 {
            lines.push(format!("  ... {hidden} more in this file"));
        }
    }
    let hidden_files = files.len() - shown_files;
    if hidden_files > 0 {
        let hidden_errors: usize = files
            .iter()
            .skip(shown_files)
            .map(|(_, items)| error_count(items))
            .sum();
        let noun = if hidden_files == 1 { "file" } else { "files" };
        lines.push(format!(
            "... {hidden_files} more {noun} with diagnostics ({hidden_errors} errors)"
        ));
    }
    lines.join("\n")
}

fn error_count(items: &[&Diagnostic]) -> usize {
    items
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

fn line(diagnostic: &Diagnostic, max_chars: usize) -> String {
    let mut text = format!(
        "  {} {}:{} {}",
        diagnostic.severity.label(),
        u64::from(diagnostic.range.start.line) + 1,
        u64::from(diagnostic.range.start.character) + 1,
        one_line(&diagnostic.message, max_chars)
    );
    if let Some(code) = diagnostic.code.as_deref().filter(|code| !code.is_empty()) {
        text.push_str(&format!(" [{code}]"));
    }
    if let Some(source) = diagnostic
        .source
        .as_deref()
        .filter(|source| !source.is_empty())
    {
        text.push_str(&format!(" ({source})"));
    }
    text
}

fn one_line(message: &str, max_chars: usize) -> String {
    let joined = message
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.chars().count() <= max_chars {
        return joined;
    }
    let mut cut: String = joined.chars().take(max_chars.saturating_sub(3)).collect();
    cut.push_str("...");
    cut
}
