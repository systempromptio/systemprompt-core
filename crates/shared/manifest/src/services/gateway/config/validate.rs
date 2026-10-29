//! Checks of the gateway section on its own: declared route ids, route shape,
//! request predicates, weights, the `[1m]` refusal and override rules.
//! References into the catalog are checked by
//! [`ResolvedCatalog::build`](crate::services::catalog::ResolvedCatalog::build).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_models::providers::has_context_variant;

use crate::services::gateway::config::{BridgeInstanceSpec, BridgeReleasesSpec, GatewayConfig};
use crate::services::gateway::error::{GatewayProfileError, GatewayResult};
use crate::services::gateway::route::GatewayRoute;

impl GatewayConfig {
    #[must_use]
    pub fn unresolved_secret_refs(&self, has_secret: impl Fn(&str) -> bool) -> Vec<String> {
        let mut unresolved = Vec::new();
        if let Some(name) = self
            .bridge_releases
            .as_ref()
            .and_then(|spec| spec.token_secret.as_deref())
            && !has_secret(name)
        {
            unresolved.push(format!("bridge_releases.token_secret={name}"));
        }
        unresolved
    }

    pub fn validate(&self) -> GatewayResult<()> {
        if let Some(model) = self
            .default_model
            .as_ref()
            .filter(|m| has_context_variant(m.as_str()))
        {
            return Err(GatewayProfileError::ContextVariantInConfig {
                field: "default_model",
                value: model.as_str().to_owned(),
            });
        }
        let mut seen = HashSet::with_capacity(self.routes.len());
        for route in &self.routes {
            if route.id.as_str().trim().is_empty() {
                return Err(GatewayProfileError::EmptyRouteId);
            }
            if !seen.insert(route.id.clone()) {
                return Err(GatewayProfileError::DuplicateRouteId {
                    id: route.id.clone(),
                });
            }
            validate_route(route)?;
        }
        for rule in &self.system_prompt_overrides {
            rule.validate()?;
        }
        if let Some(releases) = self.bridge_releases.as_ref() {
            validate_bridge_releases(releases)?;
        }
        if let Some(instance) = self.bridge_instance.as_ref() {
            validate_bridge_instance(instance)?;
        }
        Ok(())
    }
}

fn validate_bridge_instance(instance: &BridgeInstanceSpec) -> GatewayResult<()> {
    for (field, value) in [
        ("device_link_path", &instance.device_link_path),
        ("identity_path", &instance.identity_path),
    ] {
        if !value.starts_with('/') || value.starts_with("//") || value.contains("://") {
            return Err(GatewayProfileError::BridgeInstancePathNotRelative {
                field,
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_bridge_releases(releases: &BridgeReleasesSpec) -> GatewayResult<()> {
    for (field, bound) in [
        ("min_version", releases.min_version.as_ref()),
        ("max_version", releases.max_version.as_ref()),
    ] {
        if let Some(version) = bound.filter(|v| !v.build.is_empty()) {
            return Err(GatewayProfileError::BridgeVersionBuildMetadata {
                field,
                version: version.clone(),
            });
        }
    }
    let window = releases.version_window();
    if let (Some(min), Some(max)) = (&window.min, &window.max)
        && window.is_empty()
    {
        return Err(GatewayProfileError::BridgeVersionWindowInverted {
            min: min.clone(),
            max: max.clone(),
        });
    }
    if let Some(value) = releases.pinned_version.as_deref() {
        let pinned = semver::Version::parse(value).map_err(|source| {
            GatewayProfileError::BridgePinnedVersionInvalid {
                value: value.to_owned(),
                source,
            }
        })?;
        if !window.admits(&pinned) {
            return Err(GatewayProfileError::BridgePinnedVersionOutsideWindow { pinned });
        }
    }
    Ok(())
}

fn validate_route(route: &GatewayRoute) -> GatewayResult<()> {
    if route.models.is_empty() && route.alias.is_none() {
        return Err(GatewayProfileError::RouteMatchesNothing {
            route: route.id.clone(),
        });
    }
    if let Some(alias) = route.alias.as_ref()
        && has_context_variant(alias.requested.as_str())
    {
        return Err(GatewayProfileError::ContextVariantInConfig {
            field: "routes[].alias.requested",
            value: alias.requested.as_str().to_owned(),
        });
    }
    if let Some(when) = route.when.as_ref() {
        when.validate()?;
    }
    if let Some(zero) = route.deployments.iter().find(|d| d.weight() == Some(0)) {
        return Err(GatewayProfileError::RouteDeploymentWeightZero {
            route: route.id.clone(),
            endpoint: zero.endpoint().to_string(),
        });
    }
    Ok(())
}
