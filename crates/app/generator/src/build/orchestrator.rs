use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;

use super::steps::organize_css;
use super::validation::validate_build;

pub type Result<T> = std::result::Result<T, BuildError>;

#[derive(Error, Debug)]
pub enum BuildError {
    #[error("CSS organization failed: {0}")]
    CssOrganizationFailed(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process execution error: {0}")]
    ProcessError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Development,
    Production,
    Docker,
}

impl BuildMode {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Some(Self::Development),
            "production" | "prod" => Some(Self::Production),
            "docker" => Some(Self::Docker),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
            Self::Docker => "docker",
        }
    }
}

#[derive(Debug)]
pub struct BuildOrchestrator {
    web_dir: PathBuf,
    #[allow(dead_code)]
    mode: BuildMode,
}

impl BuildOrchestrator {
    #[must_use]
    pub const fn new(web_dir: PathBuf, mode: BuildMode) -> Self {
        Self { web_dir, mode }
    }

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
