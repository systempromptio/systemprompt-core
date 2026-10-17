//! The live schema state `/readyz` reads: seeded from the boot report, it is
//! unready only while an ahead migration is `@compat: breaking`.

use systemprompt_database::{SchemaAhead, SchemaInstallReport};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::SchemaState;

fn ahead(version: u32, breaks_older: bool) -> SchemaAhead {
    SchemaAhead {
        extension: ExtensionId::new("ai"),
        version,
        name: format!("m{version}"),
        breaks_older,
    }
}

#[test]
fn a_clean_boot_has_nothing_ahead() {
    let state = SchemaState::from(SchemaInstallReport::default());
    assert!(state.ahead().is_empty());
    assert!(!state.breaks_this_release());
}

#[test]
fn the_boot_ahead_set_seeds_the_live_state() {
    let state = SchemaState::from(SchemaInstallReport {
        ahead: vec![ahead(43, false)],
        ..SchemaInstallReport::default()
    });
    assert_eq!(state.ahead(), vec![ahead(43, false)]);
    assert!(!state.breaks_this_release());
    assert!(state.boot().is_ahead());
}

#[test]
fn an_ahead_contract_migration_breaks_this_release() {
    let state = SchemaState::from(SchemaInstallReport {
        ahead: vec![ahead(43, false), ahead(44, true)],
        ..SchemaInstallReport::default()
    });
    assert!(state.breaks_this_release());
}
