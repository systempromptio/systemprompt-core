//! Seams the shared layer defines: extensions, user purge, retention, jobs,
//! document codecs and templates, and the CLI's own command registrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::purge;

use crate::commands::plugins::discover_registry;
use crate::kernel::{LinkedSeamRegistration, named, owned, prioritized};

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "cli_commands",
    owner: "systemprompt-cli",
    extension: None,
    entries: || named(
        inventory::iter::<crate::kernel::CliCommandRegistration>
            .into_iter()
            .map(crate::kernel::CliCommandRegistration::path),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "extensions",
    owner: "systemprompt-extension",
    extension: None,
    entries: || named(
        discover_registry()
            .extensions()
            .iter()
            .map(|ext| ext.id().to_owned())
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "user_purge_tables",
    owner: "systemprompt-extension",
    extension: None,
    entries: || named(
        purge::registered_user_purge_tables()
            .map(|t| format!("{}:{}.{}", t.owner, t.table, t.column)),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "orphan_sweeps",
    owner: "systemprompt-extension",
    extension: None,
    entries: || named(
        purge::registered_orphan_sweeps().map(|s| format!("{}:{}", s.owner, s.table))
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "retention_holds",
    owner: "systemprompt-extension",
    extension: None,
    entries: || by_priority!(systemprompt_extension::discover_retention_holds()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "gateway_request_guards",
    owner: "systemprompt-extension",
    extension: None,
    entries: || owned(
        systemprompt_extension::GatewayGuards::registrations()
            .map(|registration| (registration.extension, registration.extension)),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "jobs",
    owner: "systemprompt-provider-contracts",
    extension: None,
    entries: || named(
        systemprompt_traits::registered_jobs().map(|registration| registration.job.name()),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "document_codecs",
    owner: "systemprompt-provider-contracts",
    extension: Some("files"),
    entries: || named(
        systemprompt_files::documents::codecs::CodecRegistry::discover()
            .formats()
            .into_iter()
            .map(systemprompt_models::documents::DocumentFormat::as_str),
    ),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "document_templates",
    owner: "systemprompt-provider-contracts",
    extension: Some("files"),
    entries: || named(
        systemprompt_files::documents::templates::TemplateRegistry::discover(Vec::new())
            .list()
            .into_iter()
            .map(|template| template.id.as_str().to_owned()),
    ),
});
