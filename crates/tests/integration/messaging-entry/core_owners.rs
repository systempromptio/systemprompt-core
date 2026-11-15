//! Owner identity of the `slack` and `teams` core declaration rows with the
//! real entry crates linked: each extension is the type its row names and
//! passes `admit_core_owner`, and the extensions this binary discovers pass
//! `admit_core_owners` together.

use systemprompt_api::services::middleware::{
    CoreDeclaration, admit_core_owner, admit_core_owners,
};
use systemprompt_extension::{Extension, ExtensionRegistry};
use systemprompt_slack_entry::SlackExtension;
use systemprompt_teams_entry::TeamsExtension;

#[test]
fn slack_and_teams_are_the_types_their_rows_name() {
    let owners: [(&dyn Extension, CoreDeclaration); 2] = [
        (&SlackExtension, CoreDeclaration::Slack),
        (&TeamsExtension, CoreDeclaration::Teams),
    ];
    for (ext, row) in owners {
        assert_eq!(Some(ext.type_name()), row.owner_type(), "{row:?}");
        admit_core_owner(ext).unwrap_or_else(|err| panic!("{row:?}: {err}"));
    }
}

#[test]
fn the_discovered_messaging_extensions_pass_the_owner_check() {
    let registry = ExtensionRegistry::discover().expect("discover");
    for id in ["slack", "teams"] {
        assert!(
            registry
                .extensions()
                .iter()
                .any(|ext| ext.metadata().id == id),
            "the owner check must see the real {id} extension"
        );
    }
    admit_core_owners(registry.extensions().iter().map(|ext| &**ext))
        .expect("every linked core owner is its row's type");
}
