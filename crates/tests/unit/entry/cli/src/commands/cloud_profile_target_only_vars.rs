#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::admin::session::list::discover_profiles;
use systemprompt_cli::cloud::deploy::DeployProfile;
use systemprompt_cli::cloud::deploy::pipeline::DeploySecretsSource;
use systemprompt_cli::cloud::doctor::{CheckStatus, run_managed};
use systemprompt_cli::cloud::profile::edit_document::ProfileEdit;
use systemprompt_cli::cloud::profile::edit_settings::edit_server_settings;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_config::LoadedProfile;
use systemprompt_manifest::ProfileType;
use systemprompt_manifest::profile::ManagedProfile;

use crate::services_profile_fixture::{ProfileTree, https_sources_block, write_tree};

const TARGET_ONLY: &str = "SP_TARGET_ONLY_EXTERNAL_URL_c41e";
const VAULT_ADDR: &str = "SP_TARGET_ONLY_VAULT_ADDR_c41e";
const TENANT: &str = "tenant-target-only";

struct CloudProfile {
    _tree: ProfileTree,
    profiles_dir: PathBuf,
    path: PathBuf,
}

fn cloud_profile() -> CloudProfile {
    cloud_profile_with_secrets(&format!(
        "secrets:\n  source: vault\n  vault:\n    address: ${{{VAULT_ADDR}}}\n    path: app\n    \
         auth:\n      method: token\n"
    ))
}

fn cloud_profile_with_secrets(secrets: &str) -> CloudProfile {
    let tree = write_tree(&https_sources_block(&[]), secrets);
    let text = std::fs::read_to_string(&tree.profile_path).expect("read fixture");
    let text = text
        .replacen(
            "target: local\n",
            &format!("target: cloud\ncloud:\n  tenant_id: {TENANT}\n"),
            1,
        )
        .replacen(
            "  external_url: http://127.0.0.1\n",
            &format!("  external_url: ${{{TARGET_ONLY}}}\n"),
            1,
        );
    let root = tree.root.display().to_string();
    let text = text
        .replace(&format!("{root}/system/web"), "/app/web")
        .replace(&format!("{root}/cache"), "/app/cache")
        .replace(&root, "/app")
        .replacen(
            "  trusted_proxies: []\n",
            "  trusted_proxies:\n    - 10.0.0.0/8\n",
            1,
        );
    assert!(text.contains(TARGET_ONLY) && text.contains("target: cloud"));
    let profiles_dir = tree.root.join(".systemprompt/profiles");
    let dir = profiles_dir.join("prod");
    std::fs::create_dir_all(&dir).expect("profile dir");
    let path = dir.join("profile.yaml");
    std::fs::write(&path, text).expect("write profile");
    CloudProfile {
        _tree: tree,
        profiles_dir,
        path,
    }
}

fn unset_target_vars() {
    unsafe {
        std::env::remove_var(TARGET_ONLY);
        std::env::remove_var(VAULT_ADDR);
    }
}

#[test]
fn deploy_reads_a_cloud_profile_whose_target_only_variable_is_unset() {
    unset_target_vars();
    let fixture = cloud_profile();

    let profile = DeployProfile::load(&fixture.path).expect("deploy loads the management view");

    assert_eq!(profile.name.as_str(), "services_fixture");
    assert_eq!(profile.target, ProfileType::Cloud);
    assert_eq!(
        profile
            .cloud
            .as_ref()
            .and_then(|c| c.tenant_id.as_ref())
            .map(|t| t.as_str().to_owned()),
        Some(TENANT.to_owned())
    );
    assert_eq!(
        profile.secrets,
        DeploySecretsSource::Vault {
            bootstrap_env: vec![VAULT_ADDR.to_owned(), "VAULT_TOKEN".to_owned()]
        }
    );
    assert_eq!(
        profile.signing_key_path,
        fixture.path.parent().unwrap().join("signing_key.pem")
    );
    let unresolved: Vec<(&str, &str)> = profile
        .managed
        .unresolved()
        .iter()
        .map(|e| (e.variable.as_str(), e.key_path.as_str()))
        .collect();
    assert!(
        unresolved.contains(&(TARGET_ONLY, "server.external_url")),
        "{unresolved:?}"
    );
}

#[tokio::test]
async fn the_deploy_preflight_does_not_block_on_a_target_only_variable() {
    unset_target_vars();
    let fixture = cloud_profile();
    let managed = ManagedProfile::load(&fixture.path).expect("management view loads");

    let report = run_managed(&managed, fixture.path.parent().unwrap(), false).await;

    assert!(!report.has_blocking());
    assert!(
        report.not_evaluated() > 0,
        "the summary is never a clean pass"
    );
    let profile_check = report
        .checks()
        .iter()
        .find(|c| c.name == "profile")
        .expect("the whole-profile check is listed");
    assert_eq!(profile_check.status, CheckStatus::NotEvaluated);
    assert!(
        profile_check
            .detail
            .contains(&format!("${{{TARGET_ONLY}}}")),
        "{}",
        profile_check.detail
    );
    let secrets_check = report
        .checks()
        .iter()
        .find(|c| c.name == "secrets")
        .expect("the secrets check is listed");
    assert_eq!(secrets_check.status, CheckStatus::NotEvaluated);
    assert!(
        secrets_check.detail.contains(&format!("${{{VAULT_ADDR}}}")),
        "{}",
        secrets_check.detail
    );
    assert!(
        !secrets_check.detail.contains(TARGET_ONLY),
        "{}",
        secrets_check.detail
    );
}

#[tokio::test]
async fn the_preflight_runs_every_check_whose_inputs_resolve() {
    unset_target_vars();
    let fixture =
        cloud_profile_with_secrets("secrets:\n  source: file\n  secrets_path: secrets.json\n");
    let dir = fixture.path.parent().unwrap();
    std::fs::write(
        dir.join("secrets.json"),
        r#"{"oauth_at_rest_pepper":"pepper","database_url":"postgres://u:p@127.0.0.1:1/none","signing_key_pem":"pem"}"#,
    )
    .unwrap();
    let managed = ManagedProfile::load(&fixture.path).expect("management view loads");

    let report = run_managed(&managed, dir, false).await;

    let status = |name: &str| {
        report
            .checks()
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.status)
            .unwrap_or_else(|| panic!("check {name} is listed"))
    };
    assert_eq!(status("secrets"), CheckStatus::Pass);
    assert_eq!(status("signing-key"), CheckStatus::Pass);
    assert_ne!(status("database"), CheckStatus::NotEvaluated);
    assert_eq!(status("profile"), CheckStatus::NotEvaluated);
    assert_eq!(status("trusted-proxies"), CheckStatus::NotEvaluated);
    assert!(report.not_evaluated() > 0);
}

#[test]
fn editing_a_field_keeps_target_only_placeholders_as_written() {
    unset_target_vars();
    let fixture = cloud_profile();
    let before = std::fs::read_to_string(&fixture.path).unwrap();
    assert!(before.contains(&format!("${{{TARGET_ONLY}}}")));

    let mut edit = ProfileEdit::open(&fixture.path).expect("edit opens the profile as written");
    let prompter = ScriptedPrompter::new(["0.0.0.0".to_owned(), String::new(), String::new()]);
    edit_server_settings(&prompter, &mut edit).expect("edit succeeds");
    assert_eq!(edit.changed(), ["server.host".to_owned()]);
    edit.save().expect("save succeeds");

    let saved = std::fs::read_to_string(&fixture.path).unwrap();
    assert!(saved.contains(&format!("${{{TARGET_ONLY}}}")), "{saved}");
    assert!(saved.contains(&format!("${{{VAULT_ADDR}}}")), "{saved}");
    assert!(!saved.contains("unresolved"), "{saved}");
    let value: serde_yaml::Value = serde_yaml::from_str(&saved).unwrap();
    assert_eq!(
        value["server"]["host"],
        serde_yaml::Value::String("0.0.0.0".to_owned())
    );
    assert_eq!(
        value["server"]["external_url"],
        serde_yaml::Value::String(format!("${{{TARGET_ONLY}}}"))
    );
}

#[test]
fn session_list_keeps_the_tenant_of_a_profile_with_a_target_only_variable() {
    unset_target_vars();
    let fixture = cloud_profile();

    let discovered = discover_profiles(&fixture.profiles_dir);

    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "prod");
    assert_eq!(
        discovered[0].tenant.as_ref().map(|t| t.as_str().to_owned()),
        Some(TENANT.to_owned())
    );
}

#[test]
fn a_section_that_needs_a_target_only_variable_names_it() {
    unset_target_vars();
    let fixture = cloud_profile();
    let managed = ManagedProfile::load(&fixture.path).expect("management view loads");

    let error = managed
        .section::<serde_yaml::Value>("server")
        .expect_err("server holds the unresolved value");

    assert!(error.to_string().contains(TARGET_ONLY), "{error}");
    assert!(error.to_string().contains("server.external_url"), "{error}");
}

#[test]
fn booting_a_cloud_profile_with_a_target_only_variable_unset_fails_naming_it() {
    unset_target_vars();
    let fixture = cloud_profile();

    let error =
        LoadedProfile::from_path(&fixture.path).expect_err("a boot must resolve every value");

    let chain = format!("{error:#}");
    assert!(chain.contains(TARGET_ONLY), "{chain}");
    assert!(chain.contains("server.external_url"), "{chain}");

    unsafe {
        std::env::set_var(TARGET_ONLY, "https://target.example.test");
        std::env::set_var(VAULT_ADDR, "https://vault.example.test");
    }
    let booted = LoadedProfile::from_path(&fixture.path);
    unset_target_vars();
    let booted = booted.expect("the same profile boots once its variables are set");
    assert_eq!(
        booted.profile().server.external_url,
        "https://target.example.test"
    );
}
