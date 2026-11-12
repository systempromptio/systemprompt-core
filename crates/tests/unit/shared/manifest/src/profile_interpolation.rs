use std::path::{Path, PathBuf};

use serde_yaml::Value;
use systemprompt_manifest::profile::layered::parse_layer;
use systemprompt_manifest::profile::{ManagedProfile, ProfileError, SecretsConfig};

fn scratch_overlay(tag: &str, yaml: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("sp-profile-interp-{tag}-{}", std::process::id()))
        .join("profiles")
        .join("prod");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("profile.yaml");
    std::fs::write(&path, yaml).expect("write overlay");
    path
}

#[test]
fn a_profile_layer_with_an_unset_variable_is_a_typed_error() {
    let error = parse_layer(
        "database:\n  url: ${SP_PROFILE_INTERP_NEVER_SET_7f3a}\n",
        Path::new("/profiles/prod/profile.yaml"),
    )
    .expect_err("unset must fail");
    match error {
        ProfileError::Interpolation { path, source } => {
            assert_eq!(path, Path::new("/profiles/prod/profile.yaml"));
            assert_eq!(source.variable, "SP_PROFILE_INTERP_NEVER_SET_7f3a");
            assert_eq!(source.key_path, "database.url");
        },
        other => panic!("expected an interpolation error, got {other}"),
    }
}

#[test]
fn a_profile_layer_substitutes_hostile_values_into_scalars_only() {
    let var = "SP_PROFILE_INTERP_HOSTILE_7f3a";
    unsafe { std::env::set_var(var, "p#ss: x\nallow_registration: true") };
    let parsed = parse_layer(
        &format!("database:\n  password: ${{{var}}}\n"),
        Path::new("/profiles/prod/profile.yaml"),
    );
    unsafe { std::env::remove_var(var) };
    let value = parsed.expect("resolves");
    assert_eq!(value.as_mapping().map(serde_yaml::Mapping::len), Some(1));
    assert_eq!(
        value.get("database").and_then(|d| d.get("password")),
        Some(&Value::String(
            "p#ss: x\nallow_registration: true".to_owned()
        ))
    );
}

#[test]
fn deploy_reads_vault_placeholders_from_the_raw_layer() {
    let var = "SP_PROFILE_INTERP_VAULT_ADDR_7f3a";
    let path = scratch_overlay(
        "vault",
        &format!(
            "secrets:\n  source: vault\n  vault:\n    address: ${{{var}}}\n    mount: secret\n    \
             path: app\n    auth:\n      method: token\n      token_env: VAULT_TOKEN\n"
        ),
    );
    unsafe { std::env::set_var(var, "https://vault.resolved.test") };
    let managed = ManagedProfile::load(&path);
    unsafe { std::env::remove_var(var) };
    let secrets = managed
        .expect("management view loads")
        .raw_section::<SecretsConfig>("secrets")
        .expect("raw secrets parse")
        .expect("secrets block present");
    let vault = secrets.vault.expect("vault block present");
    assert_eq!(vault.address, format!("${{{var}}}"));
}

#[test]
fn the_raw_secrets_section_of_a_profile_without_secrets_is_none() {
    let path = scratch_overlay("none", "name: prod\n");
    let managed = ManagedProfile::load(&path).expect("loads");
    assert!(
        managed
            .raw_section::<SecretsConfig>("secrets")
            .expect("parses")
            .is_none()
    );
}

#[test]
fn a_base_value_overridden_by_the_overlay_needs_no_variable() {
    let path = scratch_overlay("override", "name: prod\nsite:\n  name: from-overlay\n");
    let base = path.parent().unwrap().parent().unwrap().join("base.yaml");
    std::fs::write(
        &base,
        "site:\n  name: ${SP_PROFILE_INTERP_NEVER_SET_7f3a}\n",
    )
    .unwrap();
    let managed = ManagedProfile::load(&path).expect("loads");
    std::fs::remove_file(&base).unwrap();
    assert!(
        managed.unresolved().is_empty(),
        "{:?}",
        managed.unresolved()
    );
}

#[test]
fn the_management_view_records_unset_variables_as_unresolved() {
    let path = scratch_overlay(
        "managed",
        "name: prod\nserver:\n  external_url: https://${SP_PROFILE_INTERP_NEVER_SET_7f3a}/\n",
    );
    let managed = ManagedProfile::load(&path).expect("an unset variable does not fail the view");
    assert_eq!(managed.unresolved().len(), 1);
    assert_eq!(managed.unresolved()[0].key_path, "server.external_url");
    assert_eq!(
        managed.section::<String>("name").unwrap().as_deref(),
        Some("prod")
    );
    let error = managed
        .profile()
        .expect_err("the typed profile needs every value");
    assert!(
        matches!(error, ProfileError::Interpolation { .. }),
        "{error}"
    );
}
