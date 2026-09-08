//! The validation report: typed check lines, the folded verdict, and the
//! plain-text rendering the CLI prints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::verdict::{Tone, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckLevel {
    Ok,
    Warn,
    Fail,
    Info,
}

impl CheckLevel {
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Ok => Tone::Ok,
            Self::Warn => Tone::Warn,
            Self::Fail => Tone::Err,
            Self::Info => Tone::Unknown,
        }
    }
}

/// How a validation report reads as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum ValidationCode {
    Healthy,
    Attention,
    Failing,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckLine {
    pub level: CheckLevel,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationReport {
    pub lines: Vec<CheckLine>,
    pub any_failed: bool,
}

impl ValidationReport {
    #[must_use]
    pub fn verdict(&self) -> Verdict<ValidationCode> {
        let tone = Tone::fold(self.lines.iter().map(|l| l.level.tone()), Tone::Ok);
        let code = match tone {
            Tone::Err => ValidationCode::Failing,
            Tone::Warn => ValidationCode::Attention,
            Tone::Ok | Tone::Unknown | Tone::Probing => ValidationCode::Healthy,
        };
        Verdict::new(tone, code)
    }

    #[must_use]
    pub fn rendered(&self) -> String {
        let mut s = format!("{} validate\n", crate::brand::brand().binary_name);
        for line in &self.lines {
            let prefix = match line.level {
                CheckLevel::Ok => "  [ok]   ",
                CheckLevel::Warn => "  [warn] ",
                CheckLevel::Fail => "  [fail] ",
                CheckLevel::Info => "         ",
            };
            s.push_str(prefix);
            s.push_str(&line.label);
            s.push_str(": ");
            s.push_str(&line.value);
            s.push('\n');
        }
        if self.any_failed {
            s.push_str("\nResult: FAIL — one or more critical checks did not pass.\n");
        } else {
            s.push_str("\nResult: OK\n");
        }
        s
    }
}

pub(super) struct Report {
    any_failed: bool,
    lines: Vec<CheckLine>,
}

impl Report {
    pub(super) const fn new() -> Self {
        Self {
            any_failed: false,
            lines: Vec::new(),
        }
    }
    pub(super) fn ok(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Ok,
            label: label.into(),
            value: value.into(),
        });
    }
    pub(super) fn warn(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Warn,
            label: label.into(),
            value: value.into(),
        });
    }
    pub(super) fn fail(&mut self, label: &str, value: &str) {
        self.any_failed = true;
        self.lines.push(CheckLine {
            level: CheckLevel::Fail,
            label: label.into(),
            value: value.into(),
        });
    }
    pub(super) fn info(&mut self, label: &str, value: &str) {
        self.lines.push(CheckLine {
            level: CheckLevel::Info,
            label: label.into(),
            value: value.into(),
        });
    }
    pub(super) fn into_report(self) -> ValidationReport {
        ValidationReport {
            lines: self.lines,
            any_failed: self.any_failed,
        }
    }
}
