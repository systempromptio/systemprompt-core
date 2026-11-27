//! Blocking writes of a [`Projection`] into the harness-owned config dir.
//!
//! An apply replaces the whole namespace of the previous one: managed skills,
//! bundle files and the rules block are written as projected, and whatever a
//! ledger lists that is no longer projected is removed. Both ledgers are read
//! before anything is written, so a corrupt ledger fails the apply with every
//! file as it was. Every write goes through a temporary file and a rename, and
//! only when the bytes change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_device::view::SyncCounts;
use systemprompt_engine::config::{Layout, Paths};

use crate::projection::bundle::is_owned_path;
use crate::projection::error::ProjectionError;
use crate::projection::ledger::{self, AGENTS_FILE, OWNED_LEDGER, SKILLS_DIR, SKILLS_LEDGER};
use crate::projection::model::{BundleFile, Counts, Projection, SkillSet};
use crate::projection::{defaults, managed};

/// Writes projections into one config dir. Every method blocks on the
/// filesystem; async callers run it on a blocking thread. One applier per
/// config dir, used from one task at a time.
#[derive(Debug, Clone)]
pub struct Applier {
    paths: Paths,
    layout: Layout,
}

/// What an apply did: the projection's counts, and the warnings of the plan and
/// of the apply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub counts: Counts,
    pub warnings: Vec<String>,
}

impl ApplyReport {
    #[must_use]
    pub fn sync_counts(&self) -> SyncCounts {
        self.counts.sync_counts(self.warnings.clone())
    }
}

struct Managed<'p> {
    skills: Option<&'p SkillSet>,
    bundle: &'p [BundleFile],
    rules: Option<&'p str>,
}

impl Applier {
    #[must_use]
    pub const fn new(paths: Paths, layout: Layout) -> Self {
        Self { paths, layout }
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.paths.config
    }

    pub fn apply(&self, projection: &Projection) -> Result<ApplyReport, ProjectionError> {
        self.write_managed(&Managed {
            skills: projection.skills.as_ref(),
            bundle: &projection.bundle_files,
            rules: projection.rules_block.as_deref(),
        })?;
        let mut warnings = projection.warnings.clone();
        defaults::write(
            &self.paths,
            &self.layout,
            &projection.defaults,
            &mut warnings,
        )?;
        let counts = projection.counts;
        tracing::info!(
            config = %self.paths.config.display(),
            enabled = projection.enabled,
            skills = counts.skills,
            agents = counts.agents,
            mcp = counts.mcp,
            rules = counts.rules,
            warnings = warnings.len(),
            "projection applied"
        );
        Ok(ApplyReport { counts, warnings })
    }

    pub fn clear(&self) -> Result<(), ProjectionError> {
        self.write_managed(&Managed {
            skills: None,
            bundle: &[],
            rules: None,
        })?;
        tracing::info!(config = %self.paths.config.display(), "managed content cleared");
        Ok(())
    }

    fn write_managed(&self, managed: &Managed<'_>) -> Result<(), ProjectionError> {
        let config = self.paths.config.as_path();
        let skills_root = config.join(SKILLS_DIR);
        let skills_ledger = skills_root.join(SKILLS_LEDGER);
        let owned_ledger = config.join(OWNED_LEDGER);
        let previous_skills = ledger::read_skill_ids(&skills_ledger)?;
        let previous_owned = ledger::read_owned(&owned_ledger)?;
        if let Some(file) = managed
            .bundle
            .iter()
            .find(|file| !is_owned_path(&file.path))
        {
            return Err(ProjectionError::Unowned {
                path: file.path.clone(),
            });
        }
        if let Some(file) = managed
            .skills
            .into_iter()
            .flat_map(|set| set.files.iter())
            .find(|file| !ledger::is_managed_dir(&file.dir))
        {
            return Err(ProjectionError::Unowned {
                path: format!("{SKILLS_DIR}/{}", file.dir),
            });
        }
        managed::write_skills(
            &skills_root,
            &skills_ledger,
            managed.skills,
            &previous_skills,
        )?;
        managed::write_owned(config, &owned_ledger, managed.bundle, &previous_owned)?;
        managed::write_rules(&config.join(AGENTS_FILE), managed.rules)
    }
}
