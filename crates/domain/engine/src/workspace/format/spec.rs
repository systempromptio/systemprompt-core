//! What a formatter is: its spec, the configuration that selects formatters and
//! a resolved run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// One formatter and the files it applies to.
///
/// `$FILE` in the arguments is replaced by the file's absolute path, or the
/// path is appended when no argument names it; root markers must exist in the
/// file's directory or an ancestor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatterSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
    #[serde(default)]
    pub node_modules: bool,
}

impl FormatterSpec {
    pub fn handles(&self, file_name: &str) -> bool {
        let name = file_name.to_ascii_lowercase();
        self.extensions.iter().any(|extension| {
            let extension = extension.to_ascii_lowercase();
            let suffix = if extension.starts_with('.') {
                extension
            } else {
                format!(".{extension}")
            };
            name.len() > suffix.len() && name.ends_with(&suffix)
        })
    }
}

/// Which formatters a [`Formatter`](super::Formatter) considers and how it runs
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatterConfig {
    pub enabled: bool,
    pub builtins: bool,
    pub custom: Vec<FormatterSpec>,
    pub disabled: Vec<String>,
    pub timeout: Duration,
    pub search_path: Option<OsString>,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            builtins: true,
            custom: Vec::new(),
            disabled: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            search_path: None,
        }
    }
}

/// A formatter ready to run for one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFormatter {
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}
