//! Infra- and domain-layer seams: secrets sources, authz and governance,
//! marketplace, MCP and document seams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::{agents, repository};
use systemprompt_governance::authz;
use systemprompt_mcp::services::ui_renderer;

use crate::kernel::{LinkedSeamRegistration, named, prioritized, unnamed};

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "authz_hooks",
    owner: "systemprompt-governance",
    extension: Some("authz"),
    entries: || by_count!(authz::AuthzHookRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "subject_attribute_providers",
    owner: "systemprompt-governance",
    extension: Some("authz"),
    entries: || by_count!(authz::SubjectProviderRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "role_directories",
    owner: "systemprompt-governance",
    extension: Some("authz"),
    entries: || by_count!(authz::RoleDirectoryRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "governance_policies",
    owner: "systemprompt-governance",
    extension: None,
    entries: || named(
        inventory::iter::<systemprompt_governance::policy::PolicyRegistration>
            .into_iter()
            .map(|registration| registration.id),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "marketplace_filters",
    owner: "systemprompt-marketplace",
    extension: Some("managed_resources"),
    entries: || by_priority!(systemprompt_marketplace::discover_filters()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "publication_gates",
    owner: "systemprompt-marketplace",
    extension: Some("managed_resources"),
    entries: || by_priority!(systemprompt_marketplace::managed::discover_publication_gates()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "mcp_access_recorders",
    owner: "systemprompt-mcp",
    extension: Some("mcp"),
    entries: || by_count!(systemprompt_mcp::host::McpAccessRecorderRegistration),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "ui_renderers",
    owner: "systemprompt-mcp",
    extension: Some("mcp"),
    entries: || named(
        inventory::iter::<ui_renderer::UiRendererRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "artifact_themes",
    owner: "systemprompt-mcp",
    extension: Some("mcp"),
    entries: || named(
        inventory::iter::<ui_renderer::ArtifactThemeRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "document_agent_policies",
    owner: "systemprompt-files",
    extension: Some("files"),
    entries: || named(
        inventory::iter::<agents::AgentAccessPolicyRegistration>
            .into_iter()
            .map(|registration| registration.name),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "document_blob_holders",
    owner: "systemprompt-files",
    extension: Some("files"),
    entries: || named(
        inventory::iter::<repository::BlobHolder>
            .into_iter()
            .map(|holder| holder.name),
    ),
});
