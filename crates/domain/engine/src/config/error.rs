//! Typed failures. Each names the file it came from and, where the parser
//! reports one, the line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::config::permission::InvalidRule;

/// A configuration input that could not be read, parsed or accepted.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write {}", .path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}:{line}:{column}: invalid settings", .path.display())]
    Jsonc {
        path: PathBuf,
        line: usize,
        column: usize,
        #[source]
        source: jsonc_parser::errors::ParseError,
    },
    #[error("{}: settings must be a JSON object", .path.display())]
    NotAnObject { path: PathBuf },
    #[error("{}: invalid value for `{key}`", .path.display())]
    Field {
        path: PathBuf,
        key: String,
        #[source]
        source: FieldError,
    },
    #[error("{}:{line}: invalid front matter", .path.display())]
    FrontMatter {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("{}:{line}: front matter is never closed with ---", .path.display())]
    UnterminatedFrontMatter { path: PathBuf, line: usize },
    #[error("{}:2: front matter must be a mapping of keys to values", .path.display())]
    FrontMatterShape { path: PathBuf },
    #[error("{}: skill name `{name}` does not match its directory `{dir}`", .path.display())]
    SkillName {
        path: PathBuf,
        name: String,
        dir: String,
    },
    #[error("{}: instruction file not found", .path.display())]
    InstructionMissing { path: PathBuf },
    #[error("{}: invalid instruction glob `{pattern}`", .path.display())]
    Glob {
        path: PathBuf,
        pattern: String,
        #[source]
        source: globset::Error,
    },
    #[error("a settings key path needs at least one segment")]
    EmptyKey,
    #[error("the merged settings from {} files are invalid", .files.len())]
    Merged {
        files: Vec<PathBuf>,
        #[source]
        source: serde_json::Error,
    },
}

/// Why one field of a settings object or front matter block was refused.
#[derive(Debug, thiserror::Error)]
pub enum FieldError {
    #[error("expected {expected}")]
    Type { expected: &'static str },
    #[error("unknown value `{value}`, expected {expected}")]
    Value {
        value: String,
        expected: &'static str,
    },
    #[error("`{field}` is required")]
    Missing { field: &'static str },
    #[error(transparent)]
    Rule(#[from] InvalidRule),
}

impl ConfigError {
    pub(crate) fn jsonc(path: &std::path::Path, source: jsonc_parser::errors::ParseError) -> Self {
        Self::Jsonc {
            path: path.to_path_buf(),
            line: source.line_display(),
            column: source.column_display(),
            source,
        }
    }

    pub(crate) fn field(
        path: &std::path::Path,
        key: impl Into<String>,
        source: FieldError,
    ) -> Self {
        Self::Field {
            path: path.to_path_buf(),
            key: key.into(),
            source,
        }
    }

    pub(crate) fn missing(path: &std::path::Path, field: &'static str) -> Self {
        Self::field(path, field, FieldError::Missing { field })
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Read { path, .. }
            | Self::Write { path, .. }
            | Self::Jsonc { path, .. }
            | Self::NotAnObject { path }
            | Self::Field { path, .. }
            | Self::FrontMatter { path, .. }
            | Self::UnterminatedFrontMatter { path, .. }
            | Self::FrontMatterShape { path }
            | Self::SkillName { path, .. }
            | Self::InstructionMissing { path }
            | Self::Glob { path, .. } => Some(path),
            Self::EmptyKey | Self::Merged { .. } => None,
        }
    }

    pub const fn line(&self) -> Option<usize> {
        match self {
            Self::Jsonc { line, .. }
            | Self::FrontMatter { line, .. }
            | Self::UnterminatedFrontMatter { line, .. } => Some(*line),
            Self::FrontMatterShape { .. } => Some(2),
            _ => None,
        }
    }
}
