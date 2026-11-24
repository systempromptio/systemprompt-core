//! CLI-wide runtime settings: output format, verbosity, colour, and
//! interactivity.
//!
//! [`CliConfig`] holds the resolved presentation options for a CLI invocation:
//! defaults, overridden by the CLI's environment snapshot, overridden by
//! command-line flags. The [`OutputFormat`], [`VerbosityLevel`], and
//! [`ColorMode`] enums express the individual axes. A command reads the
//! resolved config from its `CommandHost`; there is no process-global
//! instance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerbosityLevel {
    Quiet,
    Normal,
    Verbose,
    Debug,
}

impl VerbosityLevel {
    pub const fn as_tracing_filter(&self) -> Option<&'static str> {
        match self {
            Self::Quiet => Some("error"),
            Self::Normal => None,
            Self::Verbose => Some("debug"),
            Self::Debug => Some("trace"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub output_format: OutputFormat,
    pub verbosity: VerbosityLevel,
    pub color_mode: ColorMode,
    pub interactive: bool,
    pub assume_terminal: bool,
    pub profile_override: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Table,
            verbosity: VerbosityLevel::Normal,
            color_mode: ColorMode::Auto,
            interactive: true,
            assume_terminal: false,
            profile_override: None,
        }
    }
}

impl CliConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    pub const fn with_verbosity(mut self, level: VerbosityLevel) -> Self {
        self.verbosity = level;
        self
    }

    pub const fn with_color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    pub const fn with_interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub const fn with_assume_terminal(mut self, assume: bool) -> Self {
        self.assume_terminal = assume;
        self
    }

    pub fn with_profile_override(mut self, profile: Option<String>) -> Self {
        self.profile_override = profile;
        self
    }

    pub fn should_use_color(&self) -> bool {
        match self.color_mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => std::io::stdout().is_terminal(),
        }
    }

    pub fn is_json_output(&self) -> bool {
        self.output_format == OutputFormat::Json
    }

    pub fn should_show_verbose(&self) -> bool {
        self.verbosity >= VerbosityLevel::Verbose
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive
            && (self.assume_terminal
                || (std::io::stdin().is_terminal() && std::io::stdout().is_terminal()))
    }

    pub const fn output_format(&self) -> OutputFormat {
        self.output_format
    }
}
