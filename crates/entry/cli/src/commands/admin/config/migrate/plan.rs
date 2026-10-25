//! Reading a legacy `services/` tree and assembling the 0.73 catalog from it.
//!
//! [`plan`] returns `None` when no file of the tree is in the pre-0.73 shape
//! (a second run is a no-op). Otherwise it reads the legacy catalog from
//! `ai/providers.yaml`, `ai/gateway.yaml` and `ai/config.yaml`, folds and
//! replays it ([`super::fold`], [`super::models`], [`super::routes`]), and
//! returns the new catalog document plus the rewritten `ai/config.yaml` and
//! root `includes:`, and the legacy map. A legacy section anywhere else is an
//! abort: migrate reads the convention's files only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_yaml::Value;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{CatalogDocument, EndpointRef, detect_legacy_shape};
use systemprompt_models::providers::without_context_variant;

use super::abort::{GATEWAY_FILE, MigrateAbort};
use super::legacy::{LegacyAi, LegacyGateway, LegacyProvider};
use super::legacy_map::LegacyMap;
use super::replay::{DEFAULT_ROUTE_PATTERN, LegacyTree};
use super::{fold, gateway, models, routes};

const CATALOG_INCLUDES: [&str; 2] = ["../ai/providers.yaml", "../ai/gateway.yaml"];

#[derive(Debug)]
pub struct MigrationPlan {
    pub document: CatalogDocument,
    pub ai_config: Option<String>,
    pub root_config: Option<String>,
    pub legacy_map: LegacyMap,
    pub notes: Vec<String>,
}

fn read_value(path: &Path) -> Result<Option<Value>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    serde_yaml::from_str(&text)
        .map(Some)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

pub fn legacy_files(services: &Path) -> Result<Vec<(PathBuf, &'static str)>> {
    let mut found = Vec::new();
    let ai = services.join("ai");
    let mut candidates: Vec<PathBuf> = vec![services.join("config").join("config.yaml")];
    if ai.is_dir() {
        for entry in
            std::fs::read_dir(&ai).with_context(|| format!("Failed to list {}", ai.display()))?
        {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    for path in candidates {
        if let Some(value) = read_value(&path)?
            && let Some(what) = detect_legacy_shape(&value)
        {
            found.push((path, what));
        }
    }
    Ok(found)
}

fn legacy_ai(value: Option<&Value>) -> Result<LegacyAi> {
    let Some(ai) = value.and_then(|v| v.get("ai")) else {
        return Ok(LegacyAi::default());
    };
    Ok(LegacyAi {
        default_provider: ai
            .get("default_provider")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        providers: ai
            .get("providers")
            .map(|p| serde_yaml::from_value(p.clone()))
            .transpose()
            .context("ai.providers is not in the 0.72 shape")?
            .unwrap_or_default(),
    })
}

fn rewrite_ai_config(
    value: Option<Value>,
    default_image_model: Option<&ModelId>,
) -> Option<String> {
    let mut value = value?;
    let ai = value.get_mut("ai").and_then(Value::as_mapping_mut)?;
    let had_legacy = ai.remove("default_provider").is_some() | ai.remove("providers").is_some();
    if let Some(model) = default_image_model {
        ai.insert(
            Value::from("default_image_model"),
            Value::from(model.as_str()),
        );
    }
    (had_legacy || default_image_model.is_some())
        .then(|| serde_yaml::to_string(&value).unwrap_or_default())
}

fn rewrite_root(services: &Path) -> Result<Option<String>> {
    let path = services.join("config").join("config.yaml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let item = line
                .trim_start()
                .strip_prefix("- ")
                .map(|v| v.trim().trim_matches(['"', '\'']));
            !item.is_some_and(|v| CATALOG_INCLUDES.contains(&v))
        })
        .collect();
    let rewritten = format!("{}\n", kept.join("\n"));
    Ok((rewritten.trim_end() != text.trim_end()).then_some(rewritten))
}

struct LegacyInput {
    tree: LegacyTree,
    ai: LegacyAi,
    ai_value: Option<Value>,
}

fn read_legacy(services: &Path, found: &[(PathBuf, &'static str)]) -> Result<LegacyInput> {
    let ai_dir = services.join("ai");
    let canonical = [
        ai_dir.join("providers.yaml"),
        ai_dir.join("gateway.yaml"),
        ai_dir.join("config.yaml"),
    ];
    if let Some((path, what)) = found.iter().find(|(p, _)| !canonical.contains(p)) {
        return Err(MigrateAbort::new(
            &path.display().to_string(),
            [*what],
            "a legacy catalog section outside ai/providers.yaml, ai/gateway.yaml and \
             ai/config.yaml; move it there first",
        )
        .into());
    }
    let section = |path: &Path, key: &str| -> Result<Option<Value>> {
        Ok(read_value(path)?.and_then(|v| v.get(key).cloned()))
    };
    let providers: Vec<LegacyProvider> = section(&canonical[0], "providers")?
        .map(serde_yaml::from_value)
        .transpose()
        .context("ai/providers.yaml is not in the 0.72 shape")?
        .unwrap_or_default();
    let gateway: Option<LegacyGateway> = section(&canonical[1], "gateway")?
        .map(serde_yaml::from_value)
        .transpose()
        .context("ai/gateway.yaml is not in the 0.72 shape")?;
    if gateway.as_ref().is_some_and(|g| g.allow_unlisted_models) {
        return Err(MigrateAbort::new(
            GATEWAY_FILE,
            ["gateway.allow_unlisted_models"],
            "forwarding unlisted models to the default provider has no 0.73 equivalent; declare \
             the models",
        )
        .into());
    }
    let ai_value = read_value(&canonical[2])?;
    Ok(LegacyInput {
        ai: legacy_ai(ai_value.as_ref())?,
        tree: LegacyTree { providers, gateway },
        ai_value,
    })
}

fn apply_multipliers(
    document: &mut CatalogDocument,
    multipliers: &std::collections::BTreeMap<EndpointRef, f64>,
) {
    for (endpoint, factor) in multipliers {
        if let Some(e) = document
            .providers
            .providers
            .get_mut(&endpoint.provider)
            .and_then(|p| p.endpoints.get_mut(&endpoint.endpoint))
        {
            e.price_multiplier = Some(*factor);
        }
    }
}

pub fn plan(services: &Path) -> Result<Option<MigrationPlan>> {
    let found = legacy_files(services)?;
    if found.is_empty() {
        return Ok(None);
    }
    let LegacyInput { tree, ai, ai_value } = read_legacy(services, &found)?;
    let folded = fold::fold(&tree.providers, &ai)?;
    let built = models::build(&tree, &folded);
    let inputs = routes::RouteInputs {
        tree: &tree,
        folded: &folded,
        models: &built.models,
        deployments: &built.deployments,
    };
    let mut gateway_routes = routes::aliases(&inputs)?;
    let policy = tree
        .gateway
        .as_ref()
        .map(|g| routes::policy(&inputs, &g.routes))
        .transpose()?
        .unwrap_or_default();
    gateway_routes.extend(policy.routes);
    let mut route_models = policy.route_models;
    if let Some(catch_all) = tree.catch_all_route_id() {
        route_models.insert(catch_all, DEFAULT_ROUTE_PATTERN.to_owned());
    }
    let defaults = gateway::Defaults::derive(&tree, &ai, &built.models, &gateway_routes)?;
    let spec = gateway::spec(&tree, &folded, gateway_routes, &defaults)?;
    let mut document = CatalogDocument {
        credentials: folded.credentials.clone(),
        providers: folded.providers.clone(),
        models: built.models,
        deployments: built.deployments,
        gateway: spec,
    };
    apply_multipliers(&mut document, &built.multipliers);
    let mut notes = policy.notes;
    notes.extend(defaults.notes);
    Ok(Some(MigrationPlan {
        document,
        ai_config: rewrite_ai_config(ai_value, defaults.image_model.as_ref()),
        root_config: rewrite_root(services)?,
        legacy_map: LegacyMap::new(&folded, route_models, built.dropped),
        notes,
    }))
}

pub(super) fn strip_variant(model: &str) -> ModelId {
    ModelId::new(without_context_variant(model))
}
