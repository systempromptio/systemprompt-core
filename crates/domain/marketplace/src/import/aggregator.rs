//! The minimal `config/config.yaml` the loader needs to see the imported tree.
//!
//! The loader roots discovery at the root `config.yaml`: skills, plugins and
//! marketplaces are found as its grandparent's children, but every other
//! resource — MCP servers, AI, scheduler, agents — is only reachable through an
//! `includes:` entry. A base tree that ships its own `config/config.yaml` owns
//! that list; when it does not, the importer writes one naming every top-level
//! YAML file in the directories it copied.
//!
//! `web/` is excluded: the web domain reads its config file directly in an
//! unwrapped shape that the aggregator cannot include.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::error::MarketplaceError;

use super::writer::Sink;

const AGGREGATOR_RELPATH: &str = "config/config.yaml";

const NON_INCLUDABLE_DIRS: &[&str] = &[
    "access-control",
    "artifacts",
    "config",
    "hooks",
    "marketplaces",
    "plugins",
    "rules",
    "skills",
    "web",
];

pub(super) fn write_aggregator(
    from: &Path,
    base_dirs: &[String],
    sink: &Sink,
) -> Result<bool, MarketplaceError> {
    let rel = Path::new(AGGREGATOR_RELPATH);
    if sink.exists(rel) {
        return Ok(false);
    }

    let mut includes: Vec<String> = Vec::new();
    for dir in base_dirs {
        if NON_INCLUDABLE_DIRS.contains(&dir.as_str()) {
            continue;
        }
        let src = from.join(super::base::BASE_DIR).join(dir);
        let Ok(read) = std::fs::read_dir(&src) else {
            continue;
        };
        let mut files: Vec<String> = read
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
            .filter_map(|p| Some(format!("../{dir}/{}", p.file_name()?.to_str()?)))
            .collect();
        files.sort();
        includes.extend(files);
    }

    let mut text = String::from("includes:\n");
    for include in &includes {
        text.push_str("  - ");
        text.push_str(include);
        text.push('\n');
    }
    if includes.is_empty() {
        text = String::from("includes: []\n");
    }
    text.push_str("settings:\n  agent_port_range: [9000, 9999]\n  mcp_port_range: [5000, 5999]\n");

    sink.write_bytes(rel, text.as_bytes())?;
    Ok(true)
}
