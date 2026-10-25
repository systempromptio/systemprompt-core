//! Writing a migration plan, only after the result has loaded.
//!
//! [`verify`] copies the services tree to a temporary directory, writes the
//! plan there and loads it through the real loader (includes, convention
//! catalog, gateway projection, full validation). [`apply`] does the same
//! write in place only after `verify` succeeded, so a plan the server would
//! refuse to boot from is never written. A tree without a root
//! `config/config.yaml` (a bare `services/ai` directory, as the golden
//! fixtures are) is verified under a synthetic root that includes only the
//! AI policy file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_config::write_catalog;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::services::ServicesConfig;

use super::legacy_map::LEGACY_MAP_FILE;
use super::plan::MigrationPlan;
use crate::commands::admin::config::services_io::write_gateway;

const SYNTHETIC_ROOT: &str = "includes:\n  - ../ai/config.yaml\n";
const SYNTHETIC_ROOT_NO_AI: &str = "{}\n";

fn write(path: &Path, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, body).with_context(|| format!("Failed to write {}", path.display()))
}

fn materialize(services: &Path, plan: &MigrationPlan) -> Result<Vec<PathBuf>> {
    let ai = services.join("ai");
    std::fs::create_dir_all(&ai).with_context(|| format!("Failed to create {}", ai.display()))?;
    write_catalog(&ai, &plan.document)
        .with_context(|| format!("Failed to write {}", ai.display()))?;
    let mut written: Vec<PathBuf> = [
        "credentials.yaml",
        "providers.yaml",
        "models.yaml",
        "deployments.yaml",
    ]
    .iter()
    .map(|name| ai.join(name))
    .collect();
    let gateway = ai.join("gateway.yaml");
    match plan.document.gateway.as_ref() {
        Some(spec) => {
            write_gateway(&ai, Some(spec))?;
            written.push(gateway);
        },
        None if gateway.exists() => {
            std::fs::remove_file(&gateway)
                .with_context(|| format!("Failed to remove {}", gateway.display()))?;
            written.push(gateway);
        },
        None => {},
    }
    if let Some(body) = plan.ai_config.as_deref() {
        write(&ai.join("config.yaml"), body)?;
        written.push(ai.join("config.yaml"));
    }
    if let Some(body) = plan.root_config.as_deref() {
        let root = services.join("config").join("config.yaml");
        write(&root, body)?;
        written.push(root);
    }
    let map = serde_yaml::to_string(&plan.legacy_map).context("legacy map does not serialize")?;
    write(&ai.join(LEGACY_MAP_FILE), &map)?;
    written.push(ai.join(LEGACY_MAP_FILE));
    Ok(written)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(from).follow_links(false) {
        let entry = entry.with_context(|| format!("Failed to walk {}", from.display()))?;
        let relative = entry
            .path()
            .strip_prefix(from)
            .unwrap_or_else(|_| entry.path());
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("Failed to create {}", target.display()))?;
        } else if entry.file_type().is_file() {
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("Failed to copy {}", entry.path().display()))?;
        }
    }
    Ok(())
}

pub fn verify(services: &Path, plan: &MigrationPlan) -> Result<ServicesConfig> {
    let scratch = tempfile::tempdir().context("Failed to create a scratch directory")?;
    let copy = scratch.path().join("services");
    copy_tree(services, &copy)?;
    materialize(&copy, plan)?;
    let root = copy.join("config").join("config.yaml");
    if !root.exists() {
        let synthetic = if copy.join("ai").join("config.yaml").exists() {
            SYNTHETIC_ROOT
        } else {
            SYNTHETIC_ROOT_NO_AI
        };
        write(&root, synthetic)?;
    }
    ConfigLoader::load_from_path(&root, None).map_err(|e| {
        anyhow::anyhow!("the migrated tree does not load, so nothing was written: {e}")
    })
}

pub fn apply(services: &Path, plan: &MigrationPlan) -> Result<Vec<PathBuf>> {
    verify(services, plan)?;
    materialize(services, plan)
}
