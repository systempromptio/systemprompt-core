//! Top-level build orchestrator: organises CSS, runs validation, reports
//! progress, and returns a typed [`BuildError`] on failure.

use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;

use super::steps::organize_css;
use super::validation::validate_build;

/// Convenience `Result` alias for the build module.
pub type Result<T> = std::result::Result<T, BuildError>;

/// Errors raised by the build pipeline.
#[derive(Error, Debug)]
pub enum BuildError {
    /// CSS reorganisation step failed (typically an `fs::copy` or
    /// `create_dir_all` failure).
    #[error("CSS organization failed: {0}")]
    CssOrganizationFailed(String),

    /// Post-build validation found a problem (missing `dist/index.html`,
    /// orphan sitemap URL, …).
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Filesystem I/O failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An external process returned a non-zero exit code.
    #[error("Process execution error: {0}")]
    ProcessError(String),

    /// Build configuration was invalid or missing.
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Selects the artefact profile produced by the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Development,
    Production,
    Docker,
}

impl BuildMode {
    /// Parse a [`BuildMode`] from a CLI string. Accepts both long
    /// (`development`, `production`) and short (`dev`, `prod`) aliases.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Some(Self::Development),
            "production" | "prod" => Some(Self::Production),
            "docker" => Some(Self::Docker),
            _ => None,
        }
    }

    /// Canonical lowercase string for this build mode.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
            Self::Docker => "docker",
        }
    }
}

/// Drives the end-to-end build of a generated site (CSS organisation +
/// post-build validation) for a given `web_dir`.
#[derive(Debug)]
pub struct BuildOrchestrator {
    web_dir: PathBuf,
    mode: BuildMode,
}

impl BuildOrchestrator {
    /// Create a new orchestrator pointing at the given web directory.
    #[must_use]
    pub const fn new(web_dir: PathBuf, mode: BuildMode) -> Self {
        Self { web_dir, mode }
    }

    /// Configured build mode.
    #[must_use]
    pub const fn mode(&self) -> BuildMode {
        self.mode
    }

    /// Run the full build (CSS organisation + validation) with progress
    /// reporting.
    pub async fn build(&self) -> Result<()> {
        let start = Instant::now();
        let pb = create_progress_bar();
        self.execute_build_steps(&pb).await?;
        finish_build(&pb, start);
        Ok(())
    }

    async fn execute_build_steps(&self, pb: &ProgressBar) -> Result<()> {
        pb.set_message("CSS Organization");
        organize_css(&self.web_dir).await?;
        pb.inc(1);

        pb.set_message("Validation");
        validate_build(&self.web_dir).await?;
        pb.inc(1);

        Ok(())
    }

    /// Run only the post-build validation step (no CSS reorganisation).
    pub async fn validate_only(&self) -> Result<()> {
        tracing::info!("Validating build");
        validate_build(&self.web_dir).await
    }
}

fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(2);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    pb
}

fn finish_build(pb: &ProgressBar, start: Instant) {
    pb.finish_with_message("Build complete");
    tracing::info!(
        duration_secs = start.elapsed().as_secs_f64(),
        "Build successful"
    );
}
