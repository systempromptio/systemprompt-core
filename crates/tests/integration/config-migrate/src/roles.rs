//! `config migrate` folds the retired `access-control/roles.yaml` into
//! `rules.yaml`, refuses what it cannot decide, and leaves a tree without one
//! untouched.

use std::path::Path;

use systemprompt_cli::commands::admin::config::migrate::roles::{ROLES_FILE, migrate, pending};
use systemprompt_governance::authz::rules_file::{RULES_FILE, declared_entities, parse_rules_doc};

fn write(root: &Path, file: &str, body: &str) {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

pub(crate) const ROLES: &str = r#"
rules:
  - entity_type: marketplace
    entity_id: commons
    default_included: true
    roles: [user]
    justification: Everyone gets the commons.
  - entity_type: marketplace
    entity_id: commons
    access: deny
    roles: [contractor]
  - entity_type: gateway_route
    entity_match: "*"
    roles: [user, admin]
"#;

#[test]
fn roles_rules_become_entity_blocks_merged_into_rules_yaml() {
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), ROLES_FILE, ROLES);
    write(
        root.path(),
        RULES_FILE,
        "entities:\n  - entity: plugin/kept\n    why: Already declared.\n    allow:\n      role: [user]\n",
    );
    assert!(pending(root.path()));

    let written = migrate(root.path()).expect("migrate").expect("written");

    assert!(!pending(root.path()), "the roles file is removed");
    let doc = parse_rules_doc(&std::fs::read_to_string(written).expect("read")).expect("valid");
    let entities = declared_entities(&doc).expect("entities");
    let keys: Vec<&str> = entities.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["gateway_route/*", "marketplace/commons", "plugin/kept"]
    );
    let commons = &entities["marketplace/commons"].canonical;
    assert!(commons.default_included);
    assert_eq!(commons.rules.len(), 2);
    assert!(commons.rules.iter().any(
        |r| r.rule_value == "contractor" && r.why == "Migrated from access-control/roles.yaml"
    ));
    assert!(
        migrate(root.path()).expect("rerun").is_none(),
        "a second run writes nothing"
    );
}

#[test]
fn an_entity_both_files_declare_stops_the_migration() {
    let root = tempfile::tempdir().expect("tempdir");
    write(root.path(), ROLES_FILE, ROLES);
    write(
        root.path(),
        RULES_FILE,
        "entities:\n  - entity: marketplace/commons\n    why: Hand-written.\n    allow:\n      role: [user]\n",
    );

    let err = migrate(root.path()).expect_err("refused");

    assert!(err.to_string().contains("marketplace/commons"), "{err}");
    assert!(
        pending(root.path()),
        "nothing is removed when the merge is refused"
    );
}

#[test]
fn a_glob_on_a_kind_that_takes_none_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    write(
        root.path(),
        ROLES_FILE,
        "rules:\n  - entity_type: plugin\n    entity_match: \"team-*\"\n    roles: [user]\n",
    );

    assert!(migrate(root.path()).is_err());
    assert!(!root.path().join(RULES_FILE).exists());
}
