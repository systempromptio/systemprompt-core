//! Owner identity of the closed core declaration table with the owning
//! crates linked: the real `analytics-http`, `content-http`,
//! `notifications-http` and `agent_runs_api` extensions are the types their
//! rows name and pass `admit_core_owner`, the extensions this binary
//! discovers pass `admit_core_owners` together, and a foreign type taking a
//! linked owner's id is still refused (the registry refuses the duplicate id
//! first in a real boot; the owner check refuses it on its own). The
//! unlinked case is covered by
//! `unit/entry/api/src/middleware/core_declaration_owners.rs`.

use systemprompt_agent_runs_entry::http::AgentRunsApiExtension;
use systemprompt_analytics_entry::extension::AnalyticsHttpExtension;
use systemprompt_api::services::middleware::{
    CoreDeclaration, MountError, admit_core_owner, admit_core_owners,
};
use systemprompt_content_entry::extension::ContentHttpExtension;
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};
use systemprompt_notifications_entry::extension::NotificationsHttpExtension;

#[derive(Debug)]
struct Impostor(&'static str);

impl Extension for Impostor {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.0,
            name: self.0,
            version: "0.0.0",
        }
    }
}

#[test]
fn the_linked_owners_are_the_types_their_rows_name() {
    let owners: [(&dyn Extension, CoreDeclaration); 4] = [
        (&AnalyticsHttpExtension, CoreDeclaration::Analytics),
        (&ContentHttpExtension, CoreDeclaration::ContentPublic),
        (&NotificationsHttpExtension, CoreDeclaration::Notifications),
        (&AgentRunsApiExtension, CoreDeclaration::AgentRuns),
    ];
    for (ext, row) in owners {
        assert_eq!(Some(ext.type_name()), row.owner_type(), "{row:?}");
        admit_core_owner(ext).unwrap_or_else(|err| panic!("{row:?}: {err}"));
    }
}

#[test]
fn the_discovered_extensions_pass_the_owner_check() {
    let registry = ExtensionRegistry::discover().expect("discover");
    assert!(
        registry
            .extensions()
            .iter()
            .any(|ext| ext.metadata().id == "analytics-http"),
        "the owner check must see the real analytics extension"
    );
    admit_core_owners(registry.extensions().iter().map(|ext| &**ext))
        .expect("every linked core owner is its row's type");
}

#[test]
fn a_foreign_type_taking_a_linked_owners_id_is_refused() {
    let err = admit_core_owner(&Impostor("analytics-http")).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreOwnerImpostor {
                extension: "analytics-http",
                owner: "systemprompt_analytics_entry::extension::AnalyticsHttpExtension",
                ..
            }
        ),
        "{err:?}"
    );
}
