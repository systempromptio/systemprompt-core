//! App-layer seams: gateway policies and adapters, SSO, MCP toolsets and
//! agent-run origins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::{LinkedSeam, named, unnamed};

pub(super) fn app_seams() -> Vec<LinkedSeam> {
    use systemprompt_gateway as gateway;
    use systemprompt_oauth_issuance::sso;
    let mut seams = vec![
        LinkedSeam::new(
            "gateway_safety_scanners",
            "systemprompt-gateway",
            named(
                inventory::iter::<gateway::SafetyScannerRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "gateway_route_selectors",
            "systemprompt-gateway",
            named(
                inventory::iter::<gateway::RouteSelectorRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "gateway_system_prompt_overrides",
            "systemprompt-gateway",
            named(
                inventory::iter::<gateway::SystemPromptOverrideRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "gateway_outbound_adapters",
            "systemprompt-gateway",
            named(
                inventory::iter::<gateway::OutboundAdapterRegistration>
                    .into_iter()
                    .map(|registration| registration.tag),
            ),
        ),
        LinkedSeam::new(
            "sso_providers",
            "systemprompt-oauth-issuance",
            by_count!(sso::SsoProviderRegistration),
        ),
        LinkedSeam::new(
            "sso_role_resolvers",
            "systemprompt-oauth-issuance",
            by_count!(sso::RoleResolverRegistration),
        ),
        LinkedSeam::new(
            "auth_event_sinks",
            "systemprompt-oauth-issuance",
            by_count!(sso::AuthEventSinkRegistration),
        ),
        LinkedSeam::new(
            "mcp_toolset_factories",
            "systemprompt-runtime",
            named(systemprompt_runtime::mcp_host::registered_toolset_ids()),
        ),
    ];
    seams.extend(agent_run_seams());
    seams
}

#[cfg(feature = "agent-runs")]
fn agent_run_seams() -> Vec<LinkedSeam> {
    vec![LinkedSeam::new(
        "agent_run_origins",
        "systemprompt-agent-runs",
        named(systemprompt_agent_runs::origin::registered()),
    )]
}

#[cfg(not(feature = "agent-runs"))]
const fn agent_run_seams() -> Vec<LinkedSeam> {
    Vec::new()
}
