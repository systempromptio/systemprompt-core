//! `admin config migrate --profiles <dir>`: the pre-0.73 profile rewrite.
//!
//! Every layer in the directory goes through
//! [`migrate_profile_layer`]: `base.yaml`, each `<env>/profile.yaml`, and each
//! legacy `<name>.secrets.profile.yaml`, which moves to `<name>/profile.yaml`.
//! An overlay is judged merged over the base, as it loads; a legacy file was
//! loaded on its own in 0.72, so it is judged on its own.
//! The rewritten layers are composed and loaded through the 0.73 loader
//! before anything is written, so a profile that would not boot is reported
//! and left untouched. The command runs without loading a profile, because
//! loading is exactly what a pre-0.73 profile cannot do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::profile_layer::{ProfileMigration, migrate_profile_layer};
use anyhow::{Context, Result, bail};
use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::env::{interpolate, read_env_optional};
use systemprompt_manifest::profile::{BASE_PROFILE_FILE, OVERLAY_PROFILE_FILE, merge_yaml};

const LEGACY_SUFFIX: &str = ".secrets.profile.yaml";

#[derive(Debug)]
pub struct LayerChange {
    pub source: PathBuf,
    pub target: PathBuf,
    pub migration: ProfileMigration,
    pub joins_base: bool,
}

#[derive(Debug)]
struct Layer {
    source: PathBuf,
    target: PathBuf,
    text: String,
    change: Option<ProfileMigration>,
    joins_base: bool,
}

impl Layer {
    fn read(source: PathBuf, target: PathBuf, base: Option<&Value>) -> Result<Self> {
        let text = std::fs::read_to_string(&source)
            .with_context(|| format!("reading {}", source.display()))?;
        let moved_down = source != target;
        let view = match base {
            Some(base) if !moved_down => {
                let own: Value = serde_yaml::from_str(&text)
                    .with_context(|| format!("parsing {}", source.display()))?;
                Some(merge_yaml(base.clone(), own))
            },
            _ => None,
        };
        let change = migrate_profile_layer(&text, view.as_ref(), moved_down)
            .with_context(|| format!("migrating {}", source.display()))?;
        Ok(Self {
            source,
            target,
            text,
            change,
            joins_base: false,
        })
    }

    fn migrated_text(&self) -> &str {
        self.change.as_ref().map_or(&self.text, |m| m.text.as_str())
    }

    fn is_pending(&self) -> bool {
        self.change.is_some() || self.source != self.target
    }
}

fn layers(dir: &Path) -> Result<(Option<Layer>, Vec<Layer>)> {
    let base_path = dir.join(BASE_PROFILE_FILE);
    let base = base_path
        .is_file()
        .then(|| Layer::read(base_path.clone(), base_path, None))
        .transpose()?;
    let base_view: Option<Value> = base
        .as_ref()
        .map(|layer| {
            serde_yaml::from_str(&layer.text)
                .with_context(|| format!("parsing {}", layer.source.display()))
        })
        .transpose()?;
    let mut overlays = Vec::new();
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if let Some(env) = name.strip_suffix(LEGACY_SUFFIX).filter(|_| path.is_file()) {
            let target = dir.join(env).join(OVERLAY_PROFILE_FILE);
            if target.exists() {
                bail!(
                    "{} and {} both exist; keep one and run the migration again",
                    path.display(),
                    target.display()
                );
            }
            let mut layer = Layer::read(path, target, None)?;
            layer.joins_base = base.is_some();
            overlays.push(layer);
        } else if path.join(OVERLAY_PROFILE_FILE).is_file() {
            let overlay = path.join(OVERLAY_PROFILE_FILE);
            overlays.push(Layer::read(overlay.clone(), overlay, base_view.as_ref())?);
        }
    }
    overlays.sort_by(|a, b| a.target.cmp(&b.target));
    Ok((base, overlays))
}

fn parse(text: &str, path: &Path) -> Result<Value> {
    let interpolated = interpolate(text, &|name| read_env_optional(name));
    let value: Value = serde_yaml::from_str(&interpolated)
        .with_context(|| format!("parsing the migrated {}", path.display()))?;
    Ok(match value {
        Value::Null => Value::Mapping(serde_yaml::Mapping::new()),
        other => other,
    })
}

fn verify(base: Option<&Layer>, overlays: &[Layer]) -> Result<()> {
    let base_value = base
        .map(|layer| parse(layer.migrated_text(), &layer.target))
        .transpose()?;
    for overlay in overlays {
        let value = parse(overlay.migrated_text(), &overlay.target)?;
        Profile::from_layers(base_value.clone(), value, &overlay.target).with_context(|| {
            format!(
                "the migrated {} does not load; nothing was written",
                overlay.target.display()
            )
        })?;
    }
    Ok(())
}

fn write(layer: &Layer) -> Result<()> {
    if let Some(parent) = layer.target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&layer.target, layer.migrated_text())
        .with_context(|| format!("writing {}", layer.target.display()))?;
    if layer.source != layer.target {
        std::fs::remove_file(&layer.source)
            .with_context(|| format!("removing {}", layer.source.display()))?;
    }
    Ok(())
}

fn into_changes(base: Option<Layer>, overlays: Vec<Layer>) -> Vec<LayerChange> {
    base.into_iter()
        .chain(overlays)
        .filter(Layer::is_pending)
        .map(|layer| LayerChange {
            migration: layer.change.clone().unwrap_or_else(|| ProfileMigration {
                text: layer.text.clone(),
                ..ProfileMigration::default()
            }),
            source: layer.source,
            target: layer.target,
            joins_base: layer.joins_base,
        })
        .collect()
}

pub fn plan(dir: &Path) -> Result<Vec<LayerChange>> {
    let (base, overlays) = layers(dir)?;
    if overlays.is_empty() {
        bail!(
            "{} holds no profile: expected <env>/{OVERLAY_PROFILE_FILE} or <name>{LEGACY_SUFFIX}",
            dir.display()
        );
    }
    verify(base.as_ref(), &overlays)?;
    Ok(into_changes(base, overlays))
}

pub fn apply(dir: &Path) -> Result<Vec<LayerChange>> {
    let (base, overlays) = layers(dir)?;
    if overlays.is_empty() {
        bail!(
            "{} holds no profile: expected <env>/{OVERLAY_PROFILE_FILE} or <name>{LEGACY_SUFFIX}",
            dir.display()
        );
    }
    verify(base.as_ref(), &overlays)?;
    for layer in base.iter().chain(&overlays).filter(|l| l.is_pending()) {
        write(layer)?;
    }
    Ok(into_changes(base, overlays))
}

pub fn describe(changes: &[LayerChange]) -> String {
    if changes.is_empty() {
        return "every profile is already in the 0.73 shape".to_owned();
    }
    let mut lines = Vec::new();
    for change in changes {
        let m = &change.migration;
        if change.source == change.target {
            lines.push(format!("{}", change.target.display()));
        } else {
            lines.push(format!(
                "{} -> {}",
                change.source.display(),
                change.target.display()
            ));
        }
        if let Some(url) = &m.external_url {
            lines.push(format!("  server.external_url: {url}"));
        }
        if !m.extra_cors_origins.is_empty() {
            lines.push(format!(
                "  server.extra_cors_origins: {}",
                m.extra_cors_origins.join(", ")
            ));
        }
        if !m.removed.is_empty() {
            lines.push(format!("  removed (now derived): {}", m.removed.join(", ")));
        }
        if m.renamed_judge {
            lines.push("  evaluation: renamed to judge:".to_owned());
        }
        if change.joins_base {
            lines.push(format!(
                "  now loads over {BASE_PROFILE_FILE}: keys this profile does not state come from the base"
            ));
        }
        if m.rooted_at_parent {
            lines.push("  paths.root: \"..\" keeps relative paths where they resolved".to_owned());
        }
    }
    lines.join("\n")
}
