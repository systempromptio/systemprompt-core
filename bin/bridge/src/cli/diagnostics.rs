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
    _ = writeln!(
        out,
        "{} {}",
        crate::brand::brand().binary_name,
        env!("CARGO_PKG_VERSION")
    );
    _ = writeln!(out, "commit:    {GIT_SHA}");
    _ = writeln!(out, "branch:    {GIT_BRANCH}");
    _ = writeln!(out, "committed: {GIT_COMMIT_DATE}");
    _ = writeln!(out, "built:     {BUILD_TIMESTAMP}");
    _ = writeln!(
        out,
        "profile:   {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    _ = writeln!(
        out,
        "os:        {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    _ = writeln!(out);
    _ = writeln!(out, "paths:");
    _ = writeln!(
        out,
        "  log dir:    {}",
        crate::obs::log_dir().map_or_else(|| "<unavailable>".into(), |p| p.display().to_string())
    );
    _ = writeln!(
        out,
        "  log file:   {}",
        crate::obs::log_file_path()
            .map_or_else(|| "<unavailable>".into(), |p| p.display().to_string())
    );
    _ = writeln!(
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
