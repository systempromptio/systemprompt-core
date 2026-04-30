use std::fmt::Write as _;
use std::process::ExitCode;

use crate::cli::output;

pub const GIT_SHA: &str = env!("VERGEN_GIT_SHA");
pub const GIT_COMMIT_DATE: &str = env!("VERGEN_GIT_COMMIT_DATE");
pub const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
pub const GIT_BRANCH: &str = env!("VERGEN_GIT_BRANCH");

#[must_use]
pub fn short_sha() -> &'static str {
    let len = GIT_SHA.len().min(7);
    &GIT_SHA[..len]
}

pub fn render() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "systemprompt-bridge {}",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(out, "commit:    {GIT_SHA}");
    let _ = writeln!(out, "branch:    {GIT_BRANCH}");
    let _ = writeln!(out, "committed: {GIT_COMMIT_DATE}");
    let _ = writeln!(out, "built:     {BUILD_TIMESTAMP}");
    let _ = writeln!(
        out,
        "profile:   {}",
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );
    let _ = writeln!(
        out,
        "os:        {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "paths:");
    let _ = writeln!(
        out,
        "  log dir:    {}",
        crate::obs::log_dir()
            .map_or_else(|| "<unavailable>".into(), |p| p.display().to_string())
    );
    let _ = writeln!(
        out,
        "  log file:   {}",
        crate::obs::log_file_path()
            .map_or_else(|| "<unavailable>".into(), |p| p.display().to_string())
    );
    let _ = writeln!(
        out,
        "  config:     {}",
        crate::config::config_path()
            .map_or_else(|| "<unavailable>".into(), |p| p.display().to_string())
    );
    out
}

pub fn cmd_diagnostics() -> ExitCode {
    output::print_str(&render());
    ExitCode::SUCCESS
}
