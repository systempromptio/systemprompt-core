//! `services validate` — load a services tree the way an instance would.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_loader::ConfigLoader;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_marketplace::CatalogContent;

use super::staging::{StagedBundle, compose_staged, stage_archive, stage_tree};
use super::versions::detect_drift;
use crate::shared::CommandOutput;

const VALIDATE_API_URL: &str = "http://localhost";
const CONFIG_RELPATH: &str = "config/config.yaml";

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(long, help = "Services tree to validate")]
    pub root: PathBuf,

    #[arg(long, help = "Base bundle to compose underneath the tree")]
    pub base: Option<PathBuf>,

    #[arg(long, help = "Previous bundle to compare plugin versions against")]
    pub against: Option<PathBuf>,

    #[arg(long, help = "Treat warnings as failures")]
    pub strict: bool,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub check: String,
    pub status: String,
    pub detail: String,
}

impl Finding {
    fn pass(check: &str, detail: impl Into<String>) -> Self {
        Self::new(check, "pass", detail)
    }

    fn warn(check: &str, detail: impl Into<String>) -> Self {
        Self::new(check, "warn", detail)
    }

    fn fail(check: &str, detail: impl Into<String>) -> Self {
        Self::new(check, "fail", detail)
    }

    fn new(check: &str, status: &str, detail: impl Into<String>) -> Self {
        Self {
            check: check.to_owned(),
            status: status.to_owned(),
            detail: detail.into(),
        }
    }
}

pub fn execute(args: &ValidateArgs) -> Result<(CommandOutput, bool)> {
    let findings = run(args)?;
    let blocking = findings.iter().any(|f| f.status == "fail")
        || (args.strict && findings.iter().any(|f| f.status == "warn"));
    let output = CommandOutput::table_of(vec!["check", "status", "detail"], &findings)
        .with_title("Services Validation");
    Ok((output, !blocking))
}

pub fn run(args: &ValidateArgs) -> Result<Vec<Finding>> {
    let scratch = tempfile::tempdir().context("Failed to create a scratch directory")?;
    let cache = BundleCache::new(scratch.path().join("cache"));

    let mut findings = Vec::new();
    let mut members: Vec<StagedBundle> = Vec::new();
    if let Some(base) = args.base.as_deref() {
        members.push(stage_archive(&cache, "base", base)?);
    }
    let tree = stage_tree(&cache, "tree", &args.root, scratch.path())?;
    let tree_root = tree.root.clone();
    let tree_manifest = tree.manifest.clone();
    members.push(tree);

    let composed = if members.len() > 1 {
        match compose_staged(&cache, &members) {
            Ok(root) => {
                findings.push(Finding::pass(
                    "compose",
                    "base and tree compose without collisions",
                ));
                root
            },
            Err(err) => {
                findings.push(Finding::fail("compose", err.to_string()));
                return Ok(findings);
            },
        }
    } else {
        tree_root
    };

    findings.extend(load_findings(&composed));

    if let Some(previous) = args.against.as_deref() {
        let staged = stage_archive(&cache, "previous", previous)?;
        let drift = detect_drift(&staged.manifest, &staged.root, &tree_manifest, &args.root)?;
        findings.push(drift_finding(&drift));
    }

    Ok(findings)
}

fn load_findings(root: &Path) -> Vec<Finding> {
    if !root.join(CONFIG_RELPATH).is_file() {
        return vec![Finding::fail(
            "config",
            format!(
                "no {CONFIG_RELPATH} in the composed root; a marketplace-only tree carries none, \
                 so validate it with --base <platform bundle>"
            ),
        )];
    }
    let services = match ConfigLoader::load_from_path(&root.join(CONFIG_RELPATH)) {
        Ok(services) => services,
        Err(err) => return vec![Finding::fail("config", err.to_string())],
    };
    let mut findings = vec![Finding::pass("config", "services config loads")];

    match CatalogContent::load(&services, root, VALIDATE_API_URL) {
        Ok(catalog) => {
            let content = catalog.as_content();
            findings.push(Finding::pass(
                "catalog",
                format!(
                    "{} skill(s), {} agent(s), {} artifact(s), {} rule(s)",
                    content.skills.len(),
                    content.agents.len(),
                    content.artifacts.len(),
                    content.rules.len()
                ),
            ));
        },
        Err(err) => findings.push(Finding::fail("catalog", err.to_string())),
    }
    findings
}

fn drift_finding(drift: &[super::versions::VersionDrift]) -> Finding {
    if drift.is_empty() {
        return Finding::pass("versions", "every changed plugin bumped its version");
    }
    let detail = drift
        .iter()
        .map(|d| {
            format!(
                "{} still at {} with {} changed file(s)",
                d.plugin, d.version, d.changed_files
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Finding::warn("versions", detail)
}
