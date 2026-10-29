//! The migrated gateway section and the defaults derived from 0.72 config.
//!
//! The one default model is `gateway.default_model` with `[1m]` stripped, else
//! the default provider's `ai.providers.<p>.default_model`; an alias resolves
//! to its model. `ai.default_image_model` comes from the default provider's
//! `default_image_model`, else from the one provider that sets it. The
//! catalog route id is the id 0.72.1 gave its `default_provider` catch-all,
//! so access rules written against it keep applying. A system-prompt override
//! naming one entry of a provider that now has several endpoints aborts: it
//! would start applying to the siblings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_manifest::services::catalog::ModelRegistry;
use systemprompt_manifest::services::{GatewayConfigSpec, GatewayRoute};

use super::abort::{AI_FILE, GATEWAY_FILE, MigrateAbort, MigrateResult};
use super::fold::Folded;
use super::legacy::LegacyAi;
use super::plan::strip_variant;
use super::replay::LegacyTree;

#[derive(Debug, Default)]
pub struct Defaults {
    pub model: Option<ModelId>,
    pub image_model: Option<ModelId>,
    pub notes: Vec<String>,
}

fn resolve_model(
    requested: &str,
    models: &ModelRegistry,
    routes: &[GatewayRoute],
    key: &str,
    file: &str,
) -> MigrateResult<ModelId> {
    let id = strip_variant(requested);
    if models.get(id.as_str()).is_some() {
        return Ok(id);
    }
    routes
        .iter()
        .find_map(|r| r.aliases(id.as_str()).cloned())
        .ok_or_else(|| {
            MigrateAbort::new(
                file,
                [key],
                format!("'{requested}' is not a model 0.72.1 could dispatch, so it cannot be the default"),
            )
        })
}

fn sole_image_model(ai: &LegacyAi) -> MigrateResult<Option<(String, String)>> {
    let set: Vec<(String, String)> = ai
        .providers
        .iter()
        .filter(|(_, c)| !c.default_image_model.is_empty())
        .map(|(n, c)| (n.clone(), c.default_image_model.clone()))
        .collect();
    if set.len() > 1 {
        return Err(MigrateAbort::new(
            AI_FILE,
            set.iter()
                .map(|(n, _)| format!("ai.providers.{n}.default_image_model")),
            "several providers name a default image model and none is the default provider; \
             keep one",
        ));
    }
    Ok(set.into_iter().next())
}

impl Defaults {
    pub fn derive(
        tree: &LegacyTree,
        ai: &LegacyAi,
        models: &ModelRegistry,
        routes: &[GatewayRoute],
    ) -> MigrateResult<Self> {
        let mut out = Self::default();
        let default_provider = ai
            .default_provider
            .as_deref()
            .and_then(|p| ai.providers.get(p).map(|c| (p, c)));
        if let Some(model) = tree
            .gateway
            .as_ref()
            .and_then(|g| g.default_model.as_deref())
        {
            out.model = Some(resolve_model(
                model,
                models,
                routes,
                "gateway.default_model",
                GATEWAY_FILE,
            )?);
        } else if let Some((name, config)) =
            default_provider.filter(|(_, c)| !c.default_model.is_empty())
        {
            let key = format!("ai.providers.{name}.default_model");
            out.model = Some(resolve_model(
                &config.default_model,
                models,
                routes,
                &key,
                AI_FILE,
            )?);
        }
        let image = default_provider
            .filter(|(_, c)| !c.default_image_model.is_empty())
            .map(|(name, c)| (name.to_owned(), c.default_image_model.clone()));
        let image = image.map_or_else(|| sole_image_model(ai), |found| Ok(Some(found)))?;
        if let Some((name, model)) = image {
            let key = format!("ai.providers.{name}.default_image_model");
            out.image_model = Some(resolve_model(&model, models, routes, &key, AI_FILE)?);
        }
        let per_provider_defaults = ai
            .providers
            .values()
            .filter(|c| !c.default_model.is_empty())
            .count();
        if per_provider_defaults > 1 {
            out.notes.push(
                "ai.providers.<p>.default_model per provider is gone: the one default is gateway.default_model".to_owned(),
            );
        }
        Ok(out)
    }
}

pub fn spec(
    tree: &LegacyTree,
    folded: &Folded,
    routes: Vec<GatewayRoute>,
    defaults: &Defaults,
) -> MigrateResult<Option<GatewayConfigSpec>> {
    let Some(legacy) = tree.gateway.as_ref() else {
        if routes.is_empty() && defaults.model.is_none() {
            return Ok(None);
        }
        return Ok(Some(GatewayConfigSpec {
            routes,
            default_model: defaults.model.clone(),
            ..GatewayConfigSpec::default()
        }));
    };
    let mut overrides = legacy.system_prompt_overrides.clone();
    for (index, rule) in overrides.iter_mut().enumerate() {
        let Some(entry) = rule.provider.as_ref().map(|p| p.as_str().to_owned()) else {
            continue;
        };
        let target = folded.entries.get(&entry).ok_or_else(|| {
            MigrateAbort::new(
                GATEWAY_FILE,
                [format!("gateway.system_prompt_overrides[{index}].provider")],
                format!("names undeclared provider '{entry}'"),
            )
        })?;
        let siblings = folded
            .entries
            .values()
            .filter(|t| t.endpoint.provider == target.endpoint.provider)
            .count();
        if siblings > 1 {
            return Err(MigrateAbort::new(
                GATEWAY_FILE,
                [format!("gateway.system_prompt_overrides[{index}].provider")],
                format!(
                    "'{entry}' is now one endpoint of provider '{}'; the override would apply to every endpoint, narrow it with `models:`",
                    target.endpoint.provider
                ),
            ));
        }
        rule.provider = Some(ProviderId::new(target.endpoint.provider.as_str()));
    }
    let defaults_spec = GatewayConfigSpec::default();
    Ok(Some(GatewayConfigSpec {
        enabled: legacy.enabled,
        routes,
        default_model: defaults.model.clone(),
        quota_fault_mode: legacy.quota_fault_mode,
        require_scopes: legacy.require_scopes.clone(),
        automatic_prompt_caching: legacy.automatic_prompt_caching,
        auth_scheme: legacy
            .auth_scheme
            .clone()
            .unwrap_or(defaults_spec.auth_scheme),
        inference_path_prefix: legacy
            .inference_path_prefix
            .clone()
            .unwrap_or(defaults_spec.inference_path_prefix),
        system_prompt_overrides: overrides,
        bridge_releases: legacy.bridge_releases.clone(),
        bridge_instance: None,
    }))
}
