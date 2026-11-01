use std::path::Path;

use systemprompt_governance::authz::AuthzError;
use systemprompt_governance::authz::rules_file::declared_marketplace_entities;
use systemprompt_governance::authz::types::Access;
use tempfile::TempDir;

const HEADER: &str = "marketplace:\n  name: Demo\n  description: demo\n  version: \"1.0.0\"\n  \
                      author:\n    name: systemprompt.io\n    email: support@systemprompt.io\n  \
                      license: MIT\n";

fn write(root: &Path, dir: &str, body: &str) {
    let path = root.join("marketplaces").join(dir);
    std::fs::create_dir_all(&path).expect("mkdir");
    std::fs::write(path.join("config.yaml"), body).expect("write");
}

fn marketplace(id: &str, access: &str) -> String {
    format!("{HEADER}  id: {id}\n{access}")
}

#[test]
fn a_missing_marketplaces_dir_declares_nothing() {
    let root = TempDir::new().expect("tempdir");
    assert!(
        declared_marketplace_entities(root.path())
            .expect("ok")
            .is_empty()
    );
}

#[test]
fn roles_and_rules_become_canonical_rows() {
    let root = TempDir::new().expect("tempdir");
    write(
        root.path(),
        "demo-dir",
        &marketplace(
            "demo-market",
            "  access:\n    default_included: true\n    justification: shared tooling\n    \
             roles: [platform_admin]\n    rules:\n      - rule_type: adfs_group\n        values: \
             [devs, ops]\n      - rule_type: project\n        values: [legacy]\n        access: \
             deny\n        justification: retired\n",
        ),
    );
    let declared = declared_marketplace_entities(root.path()).expect("declared");
    let entity = declared.get("marketplace/demo-market").expect("entity");
    assert_eq!(entity.file, "marketplaces/demo-dir/config.yaml");
    assert!(entity.owner_bundle.is_none());
    assert!(entity.canonical.default_included);

    let rows: Vec<(String, String, Access, String)> = entity
        .canonical
        .rules
        .iter()
        .map(|r| {
            (
                r.rule_type.clone(),
                r.rule_value.clone(),
                r.access,
                r.why.clone(),
            )
        })
        .collect();
    assert_eq!(rows.len(), 4);
    assert!(rows.contains(&(
        "role".to_owned(),
        "platform_admin".to_owned(),
        Access::Allow,
        "shared tooling".to_owned()
    )));
    assert!(rows.iter().any(|(t, v, a, w)| t.ends_with("adfs_group")
        && v == "ops"
        && *a == Access::Allow
        && w == "shared tooling"));
    assert!(rows.iter().any(|(t, v, a, w)| t.ends_with("project")
        && v == "legacy"
        && *a == Access::Deny
        && w == "retired"));
}

#[test]
fn undeclared_access_and_dirs_without_config_are_skipped() {
    let root = TempDir::new().expect("tempdir");
    write(root.path(), "plain", &marketplace("plain-market", ""));
    std::fs::create_dir_all(root.path().join("marketplaces/empty-dir")).expect("mkdir");
    assert!(
        declared_marketplace_entities(root.path())
            .expect("ok")
            .is_empty()
    );
}

#[test]
fn the_default_justification_applies_without_one() {
    let root = TempDir::new().expect("tempdir");
    write(
        root.path(),
        "d",
        &marketplace("default-why", "  access:\n    roles: [eng]\n"),
    );
    let declared = declared_marketplace_entities(root.path()).expect("declared");
    let rule = declared["marketplace/default-why"]
        .canonical
        .rules
        .iter()
        .next()
        .expect("rule")
        .clone();
    assert_eq!(rule.why, "declared by the marketplace's access block");
    assert!(
        !declared["marketplace/default-why"]
            .canonical
            .default_included
    );
}

#[test]
fn a_blank_role_is_refused() {
    let root = TempDir::new().expect("tempdir");
    write(
        root.path(),
        "b",
        &marketplace("blank-role", "  access:\n    roles: [\"  \"]\n"),
    );
    match declared_marketplace_entities(root.path()).expect_err("blank") {
        AuthzError::Validation(message) => assert!(message.contains("blank-role"), "{message}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_malformed_rule_type_is_refused() {
    let root = TempDir::new().expect("tempdir");
    write(
        root.path(),
        "s",
        &marketplace(
            "bad-slug",
            "  access:\n    rules:\n      - rule_type: Bad Slug\n        values: [x]\n",
        ),
    );
    let err = declared_marketplace_entities(root.path()).expect_err("slug");
    assert!(
        matches!(err, AuthzError::MarketplaceRuleType { .. }),
        "{err:?}"
    );
}

#[test]
fn unparseable_yaml_names_the_file() {
    let root = TempDir::new().expect("tempdir");
    write(root.path(), "broken", "marketplace: [not, a, map\n");
    match declared_marketplace_entities(root.path()).expect_err("parse") {
        AuthzError::ConfigParse { path, .. } => assert!(path.ends_with("broken/config.yaml")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_config_path_that_is_a_directory_is_a_file_error() {
    let root = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(root.path().join("marketplaces/odd/config.yaml")).expect("mkdir");
    let err = declared_marketplace_entities(root.path()).expect_err("dir");
    assert!(
        matches!(err, AuthzError::File { action: "read", .. }),
        "{err:?}"
    );
}

#[test]
fn a_marketplaces_path_that_is_a_file_is_a_file_error() {
    let root = TempDir::new().expect("tempdir");
    std::fs::write(root.path().join("marketplaces"), b"not a dir").expect("write");
    let err = declared_marketplace_entities(root.path()).expect_err("file");
    assert!(matches!(err, AuthzError::File { .. }), "{err:?}");
}
