//! CLI-wide runtime settings: output format, verbosity, colour, and
//! interactivity.
//!
//! [`CliConfig`] holds the resolved presentation options for a CLI invocation:
//! defaults, overridden by the [`crate::env_overrides::EnvOverrides`] snapshot
//! ([`CliConfig::resolve`]), overridden by command-line flags. The
//! [`OutputFormat`], [`VerbosityLevel`], and [`ColorMode`] enums express the
//! individual axes. The resolved config travels explicitly on
//! [`crate::context::CommandContext`]; there is no process-global instance.

use std::io::IsTerminal;

use crate::env_overrides::EnvOverrides;

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

    #[must_use]
    pub fn resolve(env: &EnvOverrides) -> Self {
        let mut config = Self::default();
        config.apply_env(env);
        config
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

    /// Treat the session as terminal-attached even when stdio is piped.
    ///
    /// Interactive flows are driven through the
    /// [`crate::interactive::Prompter`] seam in tests, where no TTY exists;
    /// this bypasses the terminal probe so a scripted prompter can reach
    /// them.
    pub const fn with_assume_terminal(mut self, assume: bool) -> Self {
        self.assume_terminal = assume;
        self
    }

    pub fn with_profile_override(mut self, profile: Option<String>) -> Self {
        self.profile_override = profile;
        self
    }

    fn apply_env(&mut self, env: &EnvOverrides) {
        if let Some(format) = &env.output_format {
            self.output_format = match format.to_lowercase().as_str() {
                "json" => OutputFormat::Json,
                "yaml" => OutputFormat::Yaml,
                "table" => OutputFormat::Table,
                _ => self.output_format,
            };
        }

        if let Some(level) = &env.log_level {
            self.verbosity = match level.to_lowercase().as_str() {
                "quiet" => VerbosityLevel::Quiet,
                "normal" => VerbosityLevel::Normal,
                "verbose" => VerbosityLevel::Verbose,
                "debug" => VerbosityLevel::Debug,
                _ => self.verbosity,
            };
        }

        if env.no_color {
            self.color_mode = ColorMode::Never;
        }

        if env.non_interactive {
            self.interactive = false;
        }
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
