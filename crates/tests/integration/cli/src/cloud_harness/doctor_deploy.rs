//! Harness tests for `cloud doctor` and the `cloud deploy` preflight path,
//! plus direct unit coverage of the public doctor checks.

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::{
    check_profile_valid, check_provider_secrets, check_required_secrets, check_signing_key,
};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_loader::ProfileLoader;

use super::{Env, enter, interactive_ctx, json_ctx};

fn write_cloud_profile(env: &Env, name: &str) -> std::path::PathBuf {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    std::fs::create_dir_all(&dir).expect("cloud profile dir");
    let base =
        std::fs::read_to_string(env.root().join(".systemprompt/profiles/local/profile.yaml"))
            .expect("read base profile");
    let rewrite = |line: &str| -> String {
        let trimmed = line.trim_start();
        for (key, value) in [
            ("system:", "/app"),
            ("services:", "/app/services"),
            ("bin:", "/app/bin"),
            ("web_path:", "/app/web"),
        ] {
            if trimmed.starts_with(key) {
                return format!("  {key} {value}");
            }
        }
        line.to_owned()
    };
    let mut in_paths = false;
    let cloud: String = base
        .replace("target: local", "target: cloud")
        .lines()
        .map(|line| {
            if !line.starts_with(' ') {
                in_paths = line.starts_with("paths:");
            }
            let out = if in_paths {
                rewrite(line)
            } else {
                line.to_owned()
            };
            format!("{out}\n")
        })
        .collect();
    std::fs::write(dir.join("profile.yaml"), cloud).expect("write cloud profile");
    dir
}

fn remove_profile(env: &Env, name: &str) {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove profile");
    }
}

#[tokio::test]
async fn doctor_reports_blocking_failures() {
    let env = enter().await;
    let dir = write_cloud_profile(&env, "doc-fail");
    std::fs::write(dir.join("secrets.json"), "{}").expect("empty secrets");

    let err = cloud::execute(
        CloudCommands::Doctor {
            profile: Some("doc-fail".to_owned()),
        },
        &json_ctx(),
    )
    .await
    .expect_err("preflight fails");
    assert!(err.to_string().contains("preflight"));

    remove_profile(&env, "doc-fail");
}

#[tokio::test]
async fn doctor_passes_with_complete_profile() {
    let env = enter().await;
    let dir = write_cloud_profile(&env, "doc-pass");
    std::fs::copy(
        env.root().join("system/signing_key.pem"),
        dir.join("signing_key.pem"),
    )
    .expect("copy signing key");
    std::fs::write(
        dir.join("secrets.json"),
        r#"{"oauth_at_rest_pepper":"test_oauth_at_rest_pepper_0123456789abcdef","database_url":"postgres://u:p@127.0.0.1:5432/x"}"#,
    )
    .expect("write full secrets");

    cloud::execute(
        CloudCommands::Doctor {
            profile: Some("doc-pass".to_owned()),
        },
        &json_ctx(),
    )
    .await
    .expect("preflight passes");

    remove_profile(&env, "doc-pass");
}

#[tokio::test]
async fn doctor_requires_profile_flag_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(CloudCommands::Doctor { profile: None }, &json_ctx())
        .await
        .expect_err("needs --profile");
    assert!(err.to_string().contains("--profile"));
}

#[tokio::test]
async fn doctor_interactive_without_cloud_profiles_bails() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Doctor { profile: None },
        &interactive_ctx(Vec::<String>::new()),
    )
    .await
    .expect_err("no deployable profiles");
    assert!(err.to_string().contains("No deployable profiles"));
}

#[tokio::test]
async fn doctor_missing_profile_name_errors() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Doctor {
            profile: Some("ghost".to_owned()),
        },
        &json_ctx(),
    )
    .await
    .expect_err("unknown profile");
    assert!(err.to_string().contains("not found"));
}

fn deploy_cmd(profile: Option<&str>, check: bool) -> CloudCommands {
    CloudCommands::Deploy {
        skip_push: false,
        profile: profile.map(str::to_owned),
        no_sync: true,
        yes: true,
        dry_run: true,
        check,
    }
}

#[tokio::test]
async fn deploy_rejects_local_profile() {
    let _env = enter().await;
    let err = cloud::execute(deploy_cmd(Some("local"), false), &json_ctx())
        .await
        .expect_err("local profile rejected");
    assert!(err.to_string().contains("local profile"));
}

#[tokio::test]
async fn deploy_check_blocks_on_failing_preflight() {
    let env = enter().await;
    let dir = write_cloud_profile(&env, "dep-check");
    std::fs::write(dir.join("secrets.json"), "{}").expect("empty secrets");

    let err = cloud::execute(deploy_cmd(Some("dep-check"), true), &json_ctx())
        .await
        .expect_err("preflight blocks deploy");
    assert!(err.to_string().contains("preflight"));

    remove_profile(&env, "dep-check");
}

#[tokio::test]
async fn deploy_check_passes_without_deploying() {
    let env = enter().await;
    let dir = write_cloud_profile(&env, "dep-pass");
    std::fs::copy(
        env.root().join("system/signing_key.pem"),
        dir.join("signing_key.pem"),
    )
    .expect("copy signing key");
    std::fs::write(
        dir.join("secrets.json"),
        r#"{"oauth_at_rest_pepper":"test_oauth_at_rest_pepper_0123456789abcdef","database_url":"postgres://u:p@127.0.0.1:5432/x"}"#,
    )
    .expect("write full secrets");

    cloud::execute(deploy_cmd(Some("dep-pass"), true), &json_ctx())
        .await
        .expect("check-only deploy passes");

    remove_profile(&env, "dep-pass");
}

#[tokio::test]
async fn deploy_requires_profile_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(deploy_cmd(None, false), &json_ctx())
        .await
        .expect_err("needs --profile");
    assert!(err.to_string().contains("--profile"));
}

#[tokio::test]
async fn doctor_check_functions_cover_pass_and_fail() {
    let env = enter().await;
    let profile = ProfileLoader::load_from_path(
        &env.root().join(".systemprompt/profiles/local/profile.yaml"),
    )
    .expect("load profile");
    let profile_dir = env.root().join(".systemprompt/profiles/local");

    let result = check_profile_valid(&profile);
    let _ = format!("{result:?}");

    let mut secrets: HashMap<String, String> = HashMap::new();
    let missing = check_required_secrets(&secrets);
    assert!(missing.detail.contains("oauth_at_rest_pepper"));

    secrets.insert("oauth_at_rest_pepper".to_owned(), "p".to_owned());
    secrets.insert(
        "internal_database_url".to_owned(),
        "postgres://x".to_owned(),
    );
    let present = check_required_secrets(&secrets);
    assert!(present.detail.contains("present"));

    let key_missing = check_signing_key(&profile, &profile_dir, &secrets);
    assert!(key_missing.detail.contains("signing key"));

    secrets.insert("signing_key_pem".to_owned(), "PEM".to_owned());
    let key_via_secret = check_signing_key(&profile, &profile_dir, &secrets);
    assert!(key_via_secret.detail.contains("secrets.json"));

    let providers = check_provider_secrets(&profile, &secrets);
    let _ = format!("{providers:?}");
}
