//! App-layer seams: gateway policies and adapters, SSO and MCP toolsets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::{LinkedSeam, named, unnamed};

pub(super) fn app_seams() -> Vec<LinkedSeam> {
    use systemprompt_gateway as gateway;
    use systemprompt_oauth_issuance::sso;
    vec![
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
            "systemprompt-mcp-host",
            named(systemprompt_mcp_host::toolsets::registered_toolset_ids()),
        ),
    ]
}

