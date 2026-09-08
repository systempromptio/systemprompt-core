//! What this binary is: commit, branch, build time, and the one-line render
//! the CLI, the GUI's About data and the diagnostic bundle all share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    let brand = crate::brand::brand();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let lines = [
        format!("{} {}", brand.binary_name, brand.version),
        format!("commit:    {GIT_SHA}"),
        format!("branch:    {GIT_BRANCH}"),
        format!("committed: {GIT_COMMIT_DATE}"),
        format!("built:     {BUILD_TIMESTAMP}"),
        format!("profile:   {profile}"),
        format!(
            "os:        {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        String::new(),
        "paths:".to_owned(),
        format!(
            "  log dir:    {}",
            path_or_unavailable(crate::obs::log_dir())
        ),
        format!(
            "  log file:   {}",
            path_or_unavailable(crate::obs::log_file_path())
        ),
        format!(
            "  config:     {}",
            path_or_unavailable(crate::config::config_path())
        ),
    ];
    lines.join("\n") + "\n"
}

fn path_or_unavailable(path: Option<std::path::PathBuf>) -> String {
    path.map_or_else(|| "<unavailable>".to_owned(), |p| p.display().to_string())
}
