//! Owner identity of the closed core declaration table: every extension row
//! names the Rust type that owns it, and an extension whose id names a row
//! but whose type is not that owner is refused at boot by `admit_core_owner`,
//! before any router is built or handed a kernel handle.
//!
//! This binary links none of the capability entry crates that own the rows
//! (`systemprompt-analytics-entry`, `-slack-entry`, `-content-entry`, …), so
//! these tests cover the case the guard exists for: the owning crate is not
//! linked and a foreign extension takes its id. The linked case is covered
//! by `integration/api/server_mount_core_owners.rs`.

use systemprompt_api::services::middleware::{
    CoreDeclaration, CoreDeclarer, MountError, admit_core_owner, admit_core_owners,
};
use systemprompt_extension::{Extension, ExtensionMetadata};

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

fn refused(id: &'static str) -> (&'static str, &'static str, &'static str) {
    match admit_core_owner(&Impostor(id)).unwrap_err() {
        MountError::CoreOwnerImpostor {
            extension,
            type_name,
            owner,
        } => (extension, type_name, owner),
        other => panic!("expected CoreOwnerImpostor for {id}, got {other:?}"),
    }
}

#[test]
fn every_extension_row_names_an_owner_type_and_only_the_server_row_does_not() {
    for row in CoreDeclaration::ALL {
        match row.declarer() {
            CoreDeclarer::Server => assert_eq!(row.owner_type(), None, "{row:?}"),
            CoreDeclarer::Extension(_) => {
                assert!(row.owner_type().is_some(), "{row:?}");
            },
        }
    }
}

#[test]
fn rows_of_one_owner_id_name_one_owner_type() {
    for row in CoreDeclaration::ALL {
        for sibling in CoreDeclaration::ALL {
            if row.declarer() == sibling.declarer() {
                assert_eq!(row.owner_type(), sibling.owner_type(), "{row:?} {sibling:?}");
            }
        }
    }
}

#[test]
fn a_foreign_extension_taking_the_analytics_id_is_refused_at_boot() {
    let (extension, type_name, owner) = refused("analytics-http");
    assert_eq!(extension, "analytics-http");
    assert!(type_name.ends_with("::Impostor"), "{type_name}");
    assert_eq!(
        owner,
        "systemprompt_analytics_entry::extension::AnalyticsHttpExtension"
    );
}

#[test]
fn a_foreign_extension_taking_the_slack_id_is_refused_at_boot() {
    let (extension, _, owner) = refused("slack");
    assert_eq!(extension, "slack");
    assert_eq!(owner, "systemprompt_slack_entry::extension::SlackExtension");
}

#[test]
fn a_foreign_extension_taking_any_core_row_id_is_refused_at_boot() {
    for row in CoreDeclaration::ALL {
        let CoreDeclarer::Extension(id) = row.declarer() else {
            continue;
        };
        let (_, _, owner) = refused(id);
        assert_eq!(Some(owner), row.owner_type(), "{row:?}");
    }
}

#[test]
fn an_install_extension_owning_no_row_is_admitted() {
    for id in ["install", "analytics", "content", "notifications", "covmount"] {
        admit_core_owner(&Impostor(id)).unwrap_or_else(|err| panic!("{id}: {err}"));
    }
}

#[test]
fn one_impostor_among_the_registered_extensions_refuses_boot() {
    let install = Impostor("install");
    let impostor = Impostor("content-http");
    let registered: [&dyn Extension; 2] = [&install, &impostor];
    let err = admit_core_owners(registered).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreOwnerImpostor { extension: "content-http", .. }
        ),
        "{err:?}"
    );
    let only_install: [&dyn Extension; 1] = [&install];
    admit_core_owners(only_install).expect("no core id is taken");
}
