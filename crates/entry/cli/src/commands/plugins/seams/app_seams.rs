//! App-layer seams: gateway policies, adapters and the route groups
//! capabilities register into the gateway surface, SSO and MCP toolsets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_gateway as gateway;
use systemprompt_oauth_issuance::sso;

use crate::kernel::{LinkedSeamRegistration, named, unnamed};

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_safety_scanners",
    owner: "systemprompt-gateway",
    extension: Some("gateway"),
    entries: || named(
        inventory::iter::<gateway::SafetyScannerRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_route_selectors",
    owner: "systemprompt-gateway",
    extension: Some("gateway"),
    entries: || named(
        inventory::iter::<gateway::RouteSelectorRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_system_prompt_overrides",
    owner: "systemprompt-gateway",
    extension: Some("gateway"),
    entries: || named(
        inventory::iter::<gateway::SystemPromptOverrideRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_outbound_adapters",
    owner: "systemprompt-gateway",
    extension: Some("gateway"),
    entries: || named(
        inventory::iter::<gateway::OutboundAdapterRegistration>
            .into_iter()
            .map(|registration| registration.tag),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "sso_providers",
    owner: "systemprompt-oauth-issuance",
    extension: Some("identity"),
    entries: || by_count!(sso::SsoProviderRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "sso_role_resolvers",
    owner: "systemprompt-oauth-issuance",
    extension: Some("identity"),
    entries: || by_count!(sso::RoleResolverRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "auth_event_sinks",
    owner: "systemprompt-oauth-issuance",
    extension: Some("identity"),
    entries: || by_count!(sso::AuthEventSinkRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "mcp_toolset_factories",
    owner: "systemprompt-mcp-host",
    extension: None,
    entries: || named(systemprompt_mcp_host::toolsets::registered_toolset_ids()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_routes",
    owner: "systemprompt-api",
    extension: Some("gateway"),
    entries: || named(
        inventory::iter::<systemprompt_api::kernel::GatewayRouteRegistration>
            .into_iter()
            .map(|registration| format!("{}:{}", registration.extension, registration.scope)),
    ),
});
