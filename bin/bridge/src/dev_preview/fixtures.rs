//! Named GUI states the preview can be driven with.
//!
//! Each fixture is one `state.snapshot` reply, held as JSON on disk under
//! `web/dev/fixtures/`. On disk rather than compiled in for the same reason the
//! assets are read from disk: a fixture is something you tweak while looking at
//! the page, and a rebuild between tweaks defeats the point.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

pub fn names(web_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(fixture_dir_of(web_root)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .map(str::to_owned)
        })
        .collect();
    out.sort();
    out
}

pub fn load(web_root: &Path, name: &str) -> Option<String> {
    // Why: the name arrives straight off the query string, so it is rejected
    // rather than joined onto a path.
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    std::fs::read_to_string(fixture_dir_of(web_root).join(format!("{name}.json"))).ok()
}

fn fixture_dir_of(web_root: &Path) -> std::path::PathBuf {
    web_root.join("dev/fixtures")
}

pub fn default_web_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("web")
}
