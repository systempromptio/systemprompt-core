//! Enumeration of every link-time registration seam compiled into this
//! binary.
//!
//! Each seam is read through its owning crate's public iterator, discover
//! function or registration type. A registration that carries a stable name,
//! id or tag is listed by it; one that carries only a priority is listed by
//! priority; one that carries only a factory is listed as `unnamed#N`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::purge;

use super::extension_seams::{brain_seams, notification_seams};
use crate::commands::plugins::discover_registry;

#[derive(Debug, Clone)]
pub struct LinkedSeam {
    pub name: &'static str,
    pub owner: &'static str,
    pub entries: Vec<String>,
}

impl LinkedSeam {
    pub(super) const fn new(name: &'static str, owner: &'static str, entries: Vec<String>) -> Self {
        Self {
            name,
            owner,
            entries,
        }
    }
}

pub(super) fn named<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut out: Vec<String> = names.into_iter().map(Into::into).collect();
    out.sort();
    out
}

pub(super) fn prioritized(priorities: impl IntoIterator<Item = i32>) -> Vec<String> {
    priorities
        .into_iter()
        .map(|priority| format!("priority={priority}"))
        .collect()
}

pub(super) fn unnamed(count: usize) -> Vec<String> {
    (1..=count).map(|n| format!("unnamed#{n}")).collect()
}

pub fn linked_seams() -> Vec<LinkedSeam> {
    let mut seams = shared_seams();
    seams.extend(brain_seams());
    seams.extend(notification_seams());
    seams.extend(infra_seams());
    seams.extend(domain_seams());
    seams.extend(app_seams());
    seams.push(LinkedSeam::new(
        "cli_commands",
        "systemprompt-cli",
        named(
            inventory::iter::<crate::kernel::CliCommandRegistration>
                .into_iter()
                .map(crate::kernel::CliCommandRegistration::path),
        ),
    ));
    seams
}

fn shared_seams() -> Vec<LinkedSeam> {
    let registry = discover_registry();
    vec![
        LinkedSeam::new(
            "extensions",
            "systemprompt-extension",
            named(registry.extensions().iter().map(|ext| ext.id().to_owned())),
        ),
        LinkedSeam::new(
            "user_purge_tables",
            "systemprompt-extension",
            named(
                purge::registered_user_purge_tables()
                    .map(|t| format!("{}:{}.{}", t.owner, t.table, t.column)),
            ),
        ),
        LinkedSeam::new(
            "orphan_sweeps",
            "systemprompt-extension",
            named(purge::registered_orphan_sweeps().map(|s| format!("{}:{}", s.owner, s.table))),
        ),
        LinkedSeam::new(
            "retention_holds",
            "systemprompt-extension",
            by_priority!(systemprompt_extension::discover_retention_holds()),
        ),
        LinkedSeam::new(
            "gateway_request_guards",
            "systemprompt-extension",
            by_count!(systemprompt_extension::GatewayRequestGuardRegistration),
        ),
        LinkedSeam::new(
            "jobs",
            "systemprompt-provider-contracts",
            named(
                inventory::iter::<&'static dyn systemprompt_traits::Job>
                    .into_iter()
                    .map(|job| job.name()),
            ),
        ),
        LinkedSeam::new(
            "document_codecs",
            "systemprompt-provider-contracts",
            named(
                systemprompt_files::documents::codecs::CodecRegistry::discover()
                    .formats()
                    .into_iter()
                    .map(systemprompt_models::documents::DocumentFormat::as_str),
            ),
        ),
        LinkedSeam::new(
            "document_templates",
            "systemprompt-provider-contracts",
            named(
                systemprompt_files::documents::templates::TemplateRegistry::discover(Vec::new())
                    .list()
                    .into_iter()
                    .map(|template| template.id.as_str().to_owned()),
            ),
        ),
    ]
}

fn infra_seams() -> Vec<LinkedSeam> {
    use systemprompt_governance::authz;
    vec![
        LinkedSeam::new(
            "secrets_sources",
            "systemprompt-config",
            named(systemprompt_config::registered_external_kinds()),
        ),
        LinkedSeam::new(
            "authz_hooks",
            "systemprompt-governance",
            by_count!(authz::AuthzHookRegistration),
        ),
        LinkedSeam::new(
            "subject_attribute_providers",
            "systemprompt-governance",
            by_count!(authz::SubjectProviderRegistration),
        ),
        LinkedSeam::new(
            "role_directories",
            "systemprompt-governance",
            by_count!(authz::RoleDirectoryRegistration),
        ),
        LinkedSeam::new(
            "governance_policies",
            "systemprompt-governance",
            named(
                inventory::iter::<systemprompt_governance::policy::PolicyRegistration>
                    .into_iter()
                    .map(|registration| registration.id),
            ),
        ),
    ]
}

fn domain_seams() -> Vec<LinkedSeam> {
    use systemprompt_files::documents::{agents, repository};
    use systemprompt_mcp::services::ui_renderer;
    vec![
        LinkedSeam::new(
            "marketplace_filters",
            "systemprompt-marketplace",
            by_priority!(systemprompt_marketplace::discover_filters()),
        ),
        LinkedSeam::new(
            "publication_gates",
            "systemprompt-marketplace",
            by_priority!(systemprompt_marketplace::managed::discover_publication_gates()),
        ),
        LinkedSeam::new(
            "mcp_access_recorders",
            "systemprompt-mcp",
            by_count!(systemprompt_mcp::host::McpAccessRecorderRegistration),
        ),
        LinkedSeam::new(
            "ui_renderers",
            "systemprompt-mcp",
            named(
                inventory::iter::<ui_renderer::UiRendererRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "artifact_themes",
            "systemprompt-mcp",
            named(
                inventory::iter::<ui_renderer::ArtifactThemeRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "document_agent_policies",
            "systemprompt-files",
            named(
                inventory::iter::<agents::AgentAccessPolicyRegistration>
                    .into_iter()
                    .map(|registration| registration.name),
            ),
        ),
        LinkedSeam::new(
            "document_blob_holders",
            "systemprompt-files",
            named(
                inventory::iter::<repository::BlobHolder>
                    .into_iter()
                    .map(|holder| holder.name),
            ),
        ),
    ]
}

fn app_seams() -> Vec<LinkedSeam> {
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
