//! The gateway section: aliases, policy routes and the defaults.
//!
//! A legacy model alias becomes an alias route (`<alias>-alias`) to its
//! model, which forwards the model's `upstream_model`. A route that only
//! routed (`model_pattern` → `provider`, fallbacks, a price or an upstream
//! name) is captured by the deployments and deleted; its id maps to the
//! `model/<model_pattern>` entity in the legacy map, which carries the rules
//! that addressed it. A route that carried policy (`when`,
//! `requires`, a non-ordered strategy) is kept under its legacy id, pinned to
//! its chain's endpoints when that chain is not the model's own. A `by_scope`
//! chain identical to the plain chain is a no-op and is dropped; one that
//! differs becomes a `when: scope` route. `allow_unlisted_models`,
//! `by_scope … unmapped: deny`, route `context_fallbacks` and route
//! `extra_headers` have no 0.73 home and abort.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{ModelId, ScopeDimension};
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_manifest::services::catalog::{DeploymentRegistry, EndpointRef, ModelRegistry};
use systemprompt_manifest::services::gateway::{
    ModelGlobs, RouteAlias, RouteDeployment, WeightedDeployment, slugify_pattern,
};

use super::abort::{GATEWAY_FILE, MigrateAbort, MigrateResult, PROVIDERS_FILE};
use super::fold::Folded;
use super::legacy::{LegacyDeployment, LegacyRoute, LegacyScopeChains, LegacyUnmapped};
use super::replay::LegacyTree;

#[derive(Debug, Default)]
pub struct RoutesOut {
    pub routes: Vec<GatewayRoute>,
    pub route_models: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub struct RouteInputs<'a> {
    pub tree: &'a LegacyTree,
    pub folded: &'a Folded,
    pub models: &'a ModelRegistry,
    pub deployments: &'a DeploymentRegistry,
}

pub fn aliases(inputs: &RouteInputs<'_>) -> MigrateResult<Vec<GatewayRoute>> {
    let mut routes = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for entry in &inputs.tree.providers {
        for model in &entry.models {
            for alias in &model.aliases {
                if inputs.models.get(alias).is_some() {
                    return Err(MigrateAbort::new(
                        PROVIDERS_FILE,
                        [format!(
                            "providers[{}].models[{}].aliases",
                            entry.name, model.id
                        )],
                        format!(
                            "alias '{alias}' is also a model id: two price cards for one model"
                        ),
                    ));
                }
                if let Some(previous) = seen.insert(alias.clone(), model.id.clone()) {
                    return Err(MigrateAbort::new(
                        PROVIDERS_FILE,
                        [
                            format!("models[{previous}].aliases"),
                            format!("models[{}].aliases", model.id),
                        ],
                        format!("alias '{alias}' names two models: two price cards for one model"),
                    ));
                }
                if inputs.models.get(&model.id).is_none() || inputs.tree.resolve(alias).is_none() {
                    continue;
                }
                let mut route = GatewayRoute::new(format!("{}-alias", slugify_pattern(alias)));
                route.label = Some(format!("{alias} (alias of {})", model.id));
                route.alias = Some(RouteAlias {
                    requested: ModelId::new(alias),
                    model: ModelId::new(&model.id),
                });
                routes.push(route);
            }
        }
    }
    Ok(routes)
}

fn pins(
    folded: &Folded,
    primary: &LegacyDeployment,
    fallbacks: &[LegacyDeployment],
) -> Vec<(EndpointRef, Option<u32>)> {
    std::iter::once(primary)
        .chain(fallbacks)
        .filter_map(|d| {
            folded
                .entries
                .get(&d.provider)
                .map(|t| (t.endpoint.clone(), d.weight))
        })
        .collect()
}

fn matched<'m>(models: &'m ModelRegistry, pattern: &str) -> Vec<&'m ModelId> {
    models
        .iter()
        .map(|(id, _)| id)
        .filter(|id| ModelGlobs(vec![pattern.to_owned()]).matches(id.as_str()))
        .collect()
}

fn plain_chain(deployments: &DeploymentRegistry, model: &ModelId) -> Vec<EndpointRef> {
    deployments
        .get(model.as_str())
        .unwrap_or_default()
        .iter()
        .map(|t| t.endpoint().clone())
        .collect()
}

fn check_pins(
    inputs: &RouteInputs<'_>,
    route: &str,
    pattern: &str,
    pinned: &[(EndpointRef, Option<u32>)],
) -> MigrateResult<()> {
    for model in matched(inputs.models, pattern) {
        let chain = plain_chain(inputs.deployments, model);
        if let Some((missing, _)) = pinned.iter().find(|(e, _)| !chain.contains(e)) {
            return Err(MigrateAbort::new(
                GATEWAY_FILE,
                [format!("gateway.routes[{route}]")],
                format!(
                    "the route sends '{model}' to '{missing}', which its plain chain never reaches; a \
                     0.73 route narrows a model's deployments and cannot add one"
                ),
            ));
        }
    }
    Ok(())
}

fn pinned_route(
    id: String,
    legacy: &LegacyRoute,
    pinned: Vec<(EndpointRef, Option<u32>)>,
) -> GatewayRoute {
    let mut route = GatewayRoute::new(id);
    route.label.clone_from(&legacy.name);
    route.description.clone_from(&legacy.description);
    route.models = ModelGlobs(vec![legacy.model_pattern.clone()]);
    route.requires.clone_from(&legacy.requires);
    route.strategy = legacy.strategy;
    route.deployments = pinned
        .into_iter()
        .map(|(endpoint, weight)| match weight {
            Some(weight) => RouteDeployment::Weighted(WeightedDeployment {
                endpoint,
                weight: Some(weight),
            }),
            None => RouteDeployment::Endpoint(endpoint),
        })
        .collect();
    route
}

fn scoped_routes(
    inputs: &RouteInputs<'_>,
    route: &LegacyRoute,
    id: &str,
    scoped: &LegacyScopeChains,
) -> MigrateResult<Vec<GatewayRoute>> {
    if scoped.unmapped == LegacyUnmapped::Deny {
        return Err(MigrateAbort::new(
            GATEWAY_FILE,
            [format!("gateway.routes[{id}].by_scope.unmapped")],
            "`unmapped: deny` refuses unlisted scope values; 0.73 has no deny-by-omission route, \
             declare an access rule instead",
        ));
    }
    let mut out = Vec::new();
    for (value, chain) in &scoped.chains {
        let noop = chain.provider == route.provider
            && chain.upstream_model == route.upstream_model
            && chain.weight == route.weight
            && chain.fallbacks == route.fallbacks
            && chain.strategy == route.strategy
            && chain.context_fallbacks.is_empty();
        if noop {
            continue;
        }
        let deployment = LegacyDeployment {
            provider: chain.provider.clone(),
            upstream_model: chain.upstream_model.clone(),
            weight: chain.weight,
        };
        let pinned = pins(inputs.folded, &deployment, &chain.fallbacks);
        let scoped_id = format!(
            "{id}-{}-{}",
            slugify_pattern(&scoped.dimension),
            slugify_pattern(value)
        );
        check_pins(inputs, &scoped_id, &route.model_pattern, &pinned)?;
        let mut policy = pinned_route(scoped_id, route, pinned);
        policy.strategy = chain.strategy;
        let mut when = route.when.clone().unwrap_or_default();
        when.scope
            .insert(ScopeDimension::new(&scoped.dimension), value.clone());
        policy.when = Some(when);
        out.push(policy);
    }
    Ok(out)
}

pub fn policy(inputs: &RouteInputs<'_>, legacy: &[LegacyRoute]) -> MigrateResult<RoutesOut> {
    let mut out = RoutesOut::default();
    for route in legacy {
        let id = route.effective_id();
        let key = || [format!("gateway.routes[{id}]")];
        if !route.extra_headers.is_empty() {
            return Err(MigrateAbort::new(
                GATEWAY_FILE,
                key(),
                "route `extra_headers` have no 0.73 home; move them to the provider",
            ));
        }
        if !route.context_fallbacks.is_empty() {
            return Err(MigrateAbort::new(
                GATEWAY_FILE,
                key(),
                "route `context_fallbacks` name providers, not models; rewrite them as model ids by hand",
            ));
        }
        if matched(inputs.models, &route.model_pattern).is_empty() {
            out.notes.push(format!(
                "route '{id}' matches no migrated model and was dropped"
            ));
            out.route_models.insert(id, route.model_pattern.clone());
            continue;
        }
        let primary = LegacyDeployment {
            provider: route.provider.clone(),
            upstream_model: route.upstream_model.clone(),
            weight: route.weight,
        };
        if let Some(scoped) = route.by_scope.as_ref() {
            out.routes
                .extend(scoped_routes(inputs, route, &id, scoped)?);
        }
        let carries_policy =
            route.when.is_some() || route.requires.is_some() || !route.strategy.is_ordered();
        if !carries_policy {
            out.route_models.insert(id, route.model_pattern.clone());
            continue;
        }
        let pinned = pins(inputs.folded, &primary, &route.fallbacks);
        check_pins(inputs, &id, &route.model_pattern, &pinned)?;
        let mut policy = pinned_route(id.clone(), route, pinned);
        policy.when.clone_from(&route.when);
        out.routes.push(policy);
    }
    Ok(out)
}
