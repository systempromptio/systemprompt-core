//! How a resolved route is described on the audit row and checked against
//! its declared governance requirements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiRequestId;
use systemprompt_manifest::services::{GatewayRoute, ProviderEntry};

use super::{DispatchError, PolicyDenied};

pub fn describe_route_match(
    route: &GatewayRoute,
    declarative: Option<String>,
    selector: Option<String>,
    scope: Option<String>,
) -> String {
    let governance = route.requires.as_ref().and_then(|r| {
        let declared = r.declared();
        (!declared.is_empty()).then(|| format!("requires:{}", declared.join(",")))
    });

    let identity = Some(format!("route:{}", route.effective_id()));
    [identity, declarative, selector, governance, scope]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(";")
}

pub fn enforce_route_requirements(
    route: &GatewayRoute,
    provider: &ProviderEntry,
    requested_model: &str,
    ai_request_id: &AiRequestId,
) -> Result<(), DispatchError> {
    let Some(requires) = route.requires.as_ref() else {
        return Ok(());
    };
    let upstream = provider.upstream_model_for(route.upstream_model.as_deref(), requested_model);
    let unmet = requires.unmet(provider.effective_governance(upstream));
    if unmet.is_empty() {
        return Ok(());
    }

    tracing::warn!(
        ai_request_id = %ai_request_id,
        route = %route.effective_id(),
        model = %upstream,
        requirements = %unmet.join(","),
        "Gateway denied: route governance requirements unmet by resolved provider/model"
    );
    Err(DispatchError::pre_audit(PolicyDenied(format!(
        "route '{}' requires [{}] which provider '{}' does not satisfy for model '{}'",
        route.effective_id(),
        unmet.join(","),
        route.provider.as_str(),
        upstream
    ))))
}
