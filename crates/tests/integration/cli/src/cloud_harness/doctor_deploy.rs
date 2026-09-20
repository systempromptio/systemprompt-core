//! Harness tests for `cloud doctor` and the `cloud deploy` preflight path,
//! plus direct unit coverage of the public doctor checks.

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::{
    check_profile_valid, check_provider_secrets, check_required_secrets, check_signing_key,
};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_identifiers::TenantId;
use systemprompt_loader::ProfileLoader;

use super::{Env, TENANT_ID, enter, interactive_ctx, json_ctx, seed_tenants};

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
        .replacen("name: local", &format!("name: {name}"), 1)
        .replace("target: local", "target: cloud")
        .replace("trusted_proxies: []", "trusted_proxies: [\"fc00::/7\"]")
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
            distributed: false,
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
            distributed: false,
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
    let err = cloud::execute(
        CloudCommands::Doctor {
            profile: None,
            distributed: false,
        },
        &json_ctx(),
    )
    .await
    .expect_err("needs --profile");
    assert!(err.to_string().contains("--profile"));
}

#[tokio::test]
async fn doctor_interactive_without_cloud_profiles_bails() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Doctor {
            profile: None,
            distributed: false,
        },
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
            distributed: false,
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

    let providers = check_provider_secrets(
        &systemprompt_models::services::ProviderRegistry::default_seed().unwrap(),
        &secrets,
    );
    let _ = format!("{providers:?}");
}

fn write_cloud_profile_with_hook(env: &Env, name: &str, hook_url: &str) {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    write_cloud_profile(env, name);
    std::fs::copy(
        env.root().join("system/signing_key.pem"),
        dir.join("signing_key.pem"),
    )
    .expect("copy signing key");
    std::fs::write(
        dir.join("secrets.json"),
        r#"{"oauth_at_rest_pepper":"test_oauth_at_rest_pepper_0123456789abcdef","database_url":"postgres://u:p@127.0.0.1:5432/x"}"#,
    )
    .expect("write secrets");
    let profile_path = dir.join("profile.yaml");
    let base = std::fs::read_to_string(&profile_path).expect("read profile");
    let with_hook = base.replace(
        "      mode: unrestricted",
        &format!("      mode: unrestricted\n      url: {hook_url}"),
    );
    std::fs::write(&profile_path, with_hook).expect("write hooked profile");
}

async fn run_doctor(name: &str) {
    cloud::execute(
        CloudCommands::Doctor {
            profile: Some(name.to_owned()),
            distributed: false,
        },
        &json_ctx(),
    )
    .await
    .expect("doctor run");
}

#[tokio::test]
async fn doctor_hook_url_variants_warn_but_pass() {
    let env = enter().await;

    write_cloud_profile_with_hook(&env, "hook-loop", "http://127.0.0.1:9999/hook");
    run_doctor("hook-loop").await;
    remove_profile(&env, "hook-loop");

    write_cloud_profile_with_hook(&env, "hook-other", "https://elsewhere.example.net/hook");
    run_doctor("hook-other").await;
    remove_profile(&env, "hook-other");

    write_cloud_profile_with_hook(&env, "hook-match", "http://127.0.0.1/hook");
    run_doctor("hook-match").await;
    remove_profile(&env, "hook-match");
}

#[tokio::test]
async fn deploy_target_requires_a_synced_tenant_cache_then_resolves_the_owned_tenant() {
    use systemprompt_cli::cloud::deploy::resolve_deploy_target;
    use systemprompt_cloud::{CloudPath, get_cloud_paths};

    let env = enter().await;
    let profile_dir = write_cloud_profile(&env, "target-cache-recovery");
    let profile = ProfileLoader::load_from_path(&profile_dir.join("profile.yaml"))
        .expect("load owned cloud profile");
    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    if tenants_path.exists() {
        std::fs::remove_file(&tenants_path).expect("remove owned tenant cache");
    }

    let missing = resolve_deploy_target(&profile)
        .expect_err("deploy target resolution requires the synchronized tenant cache");
    assert!(
        format!("{missing:#}").contains("Tenants not synced"),
        "the recovery message must identify the missing synchronization step: {missing:#}"
    );

    seed_tenants(env.root());
    let target = resolve_deploy_target(&profile).expect("resolve after tenant synchronization");
    assert_eq!(target.tenant_id.as_str(), TENANT_ID);
    assert_eq!(target.tenant_name, "Harness Prod");
    assert_eq!(target.hostname.as_deref(), Some("harness.example.com"));

    remove_profile(&env, "target-cache-recovery");
}

#[tokio::test]
async fn deploy_target_rejects_a_stale_profile_tenant_then_accepts_the_repaired_identity() {
    use systemprompt_cli::cloud::deploy::resolve_deploy_target;

    let env = enter().await;
    seed_tenants(env.root());
    let profile_dir = write_cloud_profile(&env, "target-identity-recovery");
    let mut profile = ProfileLoader::load_from_path(&profile_dir.join("profile.yaml"))
        .expect("load owned cloud profile");
    profile
        .cloud
        .as_mut()
        .expect("cloud profile configuration")
        .tenant_id = Some(TenantId::new("tenant-removed-upstream"));

    let stale = resolve_deploy_target(&profile)
        .expect_err("a stale tenant identity must not select another cached tenant");
    assert!(
        format!("{stale:#}").contains("Tenant tenant-removed-upstream not found"),
        "the error must identify the stale tenant id: {stale:#}"
    );

    profile
        .cloud
        .as_mut()
        .expect("cloud profile configuration")
        .tenant_id = Some(TenantId::new(TENANT_ID));
    let repaired = resolve_deploy_target(&profile).expect("resolve repaired tenant identity");
    assert_eq!(repaired.tenant_id.as_str(), TENANT_ID);
    assert_eq!(repaired.tenant_name, "Harness Prod");
    assert_eq!(repaired.hostname.as_deref(), Some("harness.example.com"));

    remove_profile(&env, "target-identity-recovery");
}
#[cfg(unix)]
#[tokio::test]
async fn deploy_skip_push_builds_owned_image_syncs_secrets_and_requests_deploy() {
    use std::os::unix::fs::PermissionsExt;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    struct EnvGuard {
        path: Option<std::ffi::OsString>,
        docker_log: Option<std::ffi::OsString>,
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                match self.path.take() {
                    Some(path) => std::env::set_var("PATH", path),
                    None => std::env::remove_var("PATH"),
                }
                match self.docker_log.take() {
                    Some(path) => std::env::set_var("OWNED_DOCKER_LOG", path),
                    None => std::env::remove_var("OWNED_DOCKER_LOG"),
                }
            }
        }
    }

    let env = enter().await;
    env.server().reset().await;
    seed_tenants(env.root());
    let name = "deploy-owned-pipeline";
    let profile_dir = write_cloud_profile(&env, name);
    std::fs::copy(
        env.root().join("system/signing_key.pem"),
        profile_dir.join("signing_key.pem"),
    )
    .expect("copy owned signing key");
    std::fs::write(
        profile_dir.join("secrets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "oauth_at_rest_pepper": "owned_deploy_pepper_at_least_thirty_two_bytes",
            "database_url": "postgres://owned:owned@127.0.0.1:1/owned",
            "custom_marker": "owned-deploy-secret"
        }))
        .expect("serialize owned deploy secrets"),
    )
    .expect("write owned deploy secrets");

    for path in [
        env.root().join("target/release"),
        env.root().join("storage/files"),
        env.root().join("web/dist"),
        env.root().join("services/web/templates"),
        profile_dir.join("docker"),
    ] {
        std::fs::create_dir_all(path).expect("create deploy artifact directory");
    }
    std::fs::write(
        env.root().join("target/release/systemprompt"),
        "owned binary",
    )
    .expect("write owned release artifact");
    systemprompt_cli::cloud::profile::templates::save_dockerfile(
        &profile_dir.join("docker/Dockerfile"),
        name,
        env.root(),
    )
    .expect("write validated profile Dockerfile");

    let shim_dir = tempfile::tempdir().expect("owned docker shim directory");
    let docker_log = shim_dir.path().join("docker.log");
    let docker = shim_dir.path().join("docker");
    std::fs::write(
        &docker,
        "#!/bin/sh\nprintf 'PWD=%s ARGS=%s\\n' \"$PWD\" \"$*\" >> \"$OWNED_DOCKER_LOG\"\ncase \"$1\" in\n  version) printf '99.0.0\\n'; exit 0 ;;\n  build) exit 0 ;;\n  *) exit 73 ;;\nesac\n",
    )
    .expect("write owned docker shim");
    let mut permissions = std::fs::metadata(&docker).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&docker, permissions).expect("make docker shim executable");
    let previous_path = std::env::var_os("PATH");
    let _env_guard = EnvGuard {
        path: previous_path.clone(),
        docker_log: std::env::var_os("OWNED_DOCKER_LOG"),
    };
    let mut entries = vec![shim_dir.path().to_path_buf()];
    if let Some(path) = previous_path {
        entries.extend(std::env::split_paths(&path));
    }
    unsafe {
        std::env::set_var("PATH", std::env::join_paths(entries).expect("owned PATH"));
        std::env::set_var("OWNED_DOCKER_LOG", &docker_log);
    }

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "owned-tenant-bearer",
            "expires_in": 600
        })))
        .mount(env.server())
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/registry-token")))
        .and(header("authorization", "Bearer owned-tenant-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "registry": "registry.owned.invalid",
                "username": "owned-user",
                "token": "owned-registry-token",
                "repository": "systemprompt/owned",
                "tag": "coverage"
            }
        })))
        .expect(1)
        .mount(env.server())
        .await;
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .and(header("authorization", "Bearer owned-tenant-bearer"))
        .respond_with(ResponseTemplate::new(204))
        .expect(3)
        .mount(env.server())
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/deploy")))
        .and(header("authorization", "Bearer owned-tenant-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"status": "deploying", "app_url": "https://owned.example.invalid"}
        })))
        .expect(1)
        .mount(env.server())
        .await;

    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cloud::execute(
            CloudCommands::Deploy {
                skip_push: true,
                profile: Some(name.to_owned()),
                check: false,
            },
            &json_ctx(),
        ),
    )
    .await
    .expect("owned deployment completes within its bound")
    .expect("owned skip-push deployment completes");

    let docker_calls = std::fs::read_to_string(&docker_log).expect("docker shim calls");
    let image = "registry.owned.invalid/systemprompt/owned:coverage";
    let docker_lines = docker_calls.lines().collect::<Vec<_>>();
    let version_call = format!(
        "PWD={} ARGS=version --format {{{{.Server.Version}}}}",
        env.root().display()
    );
    let build_call = format!(
        "PWD={} ARGS=build --no-cache -f {} -t {image} .",
        env.root().display(),
        profile_dir.join("docker/Dockerfile").display()
    );
    assert_eq!(docker_lines, [version_call.as_str(), build_call.as_str()]);
    assert!(
        !docker_calls.contains("ARGS=login"),
        "--skip-push must not log in: {docker_calls}"
    );
    assert!(
        !docker_calls.contains("ARGS=push"),
        "--skip-push must not push: {docker_calls}"
    );

    let requests = env
        .server()
        .received_requests()
        .await
        .expect("recorded cloud requests");
    let secret_bodies = requests
        .iter()
        .filter(|request| {
            request.method.as_str() == "PUT" && request.url.path().ends_with("/secrets")
        })
        .map(|request| {
            serde_json::from_slice::<serde_json::Value>(&request.body).expect("secret JSON")
        })
        .collect::<Vec<_>>();
    assert_eq!(secret_bodies.len(), 3);
    let file_secrets = secret_bodies
        .iter()
        .find(|body| body["secrets"]["CUSTOM_MARKER"] == "owned-deploy-secret")
        .expect("file-backed secret request");
    let encoded_signing_key = file_secrets["secrets"]["SIGNING_KEY_PEM"]
        .as_str()
        .filter(|value| !value.is_empty())
        .expect("base64 signing key in file-backed secrets");
    let expected_file_secrets = serde_json::json!({"secrets": {
        "OAUTH_AT_REST_PEPPER": "owned_deploy_pepper_at_least_thirty_two_bytes",
        "DATABASE_URL": "postgres://owned:owned@127.0.0.1:1/owned",
        "CUSTOM_MARKER": "owned-deploy-secret",
        "SYSTEMPROMPT_CUSTOM_SECRETS": "CUSTOM_MARKER",
        "SIGNING_KEY_PEM": encoded_signing_key
    }});
    let expected_credentials = serde_json::json!({"secrets": {
        "SYSTEMPROMPT_API_TOKEN": super::FAR_FUTURE_JWT,
        "SYSTEMPROMPT_USER_EMAIL": super::USER_EMAIL,
        "SYSTEMPROMPT_CLI_REMOTE": "true"
    }});
    let expected_profile = serde_json::json!({"secrets": {
        "SYSTEMPROMPT_PROFILE": format!("/app/services/profiles/{name}/profile.yaml")
    }});
    assert!(
        secret_bodies.contains(&expected_file_secrets),
        "file-backed secret request must contain only the expected mapped keys"
    );
    assert!(
        secret_bodies.contains(&expected_credentials),
        "credential request must contain only the expected remote credentials"
    );
    assert!(
        secret_bodies.contains(&expected_profile),
        "profile request must contain only the container profile path"
    );
    let deploy = requests
        .iter()
        .find(|request| {
            request.method.as_str() == "POST" && request.url.path().ends_with("/deploy")
        })
        .expect("deploy request");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&deploy.body).expect("deploy JSON"),
        serde_json::json!({"image": image})
    );

    remove_profile(&env, name);
}
// Append to crates/tests/integration/cli/src/cloud_harness/doctor_deploy.rs

#[cfg(unix)]
struct OwnedDeployDocker {
    _dir: tempfile::TempDir,
    log: std::path::PathBuf,
    push_marker: std::path::PathBuf,
    previous_path: Option<std::ffi::OsString>,
    previous_log: Option<std::ffi::OsString>,
    previous_push_marker: Option<std::ffi::OsString>,
}

#[cfg(unix)]
impl OwnedDeployDocker {
    fn install(build_exit: i32) -> Self {
        Self::install_script(&format!(
            "case \"$1\" in\n  version) printf '99.0.0\\n'; exit 0 ;;\n  build) exit {build_exit} ;;\n  *) exit 73 ;;\nesac\n"
        ))
    }

    fn install_push() -> Self {
        Self::install_script(
            "case \"$1\" in\n  version) printf '99.0.0\\n'; exit 0 ;;\n  build) exit 0 ;;\n  login) IFS= read -r password; printf 'STDIN=%s\\n' \"$password\" >> \"$OWNED_DOCKER_LOG\"; exit 0 ;;\n  push) : > \"$OWNED_PUSH_MARKER\"; exit 0 ;;\n  *) exit 73 ;;\nesac\n",
        )
    }

    fn install_script(dispatch: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("owned Docker shim directory");
        let log = dir.path().join("docker.log");
        let push_marker = dir.path().join("push-completed");
        let docker = dir.path().join("docker");
        std::fs::write(
            &docker,
            format!("#!/bin/sh\nprintf 'PWD=%s ARGS=%s\\n' \"$PWD\" \"$*\" >> \"$OWNED_DOCKER_LOG\"\n{dispatch}"),
        )
        .expect("write owned Docker shim");
        let mut permissions = std::fs::metadata(&docker)
            .expect("Docker shim metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&docker, permissions).expect("make Docker shim executable");

        let previous_path = std::env::var_os("PATH");
        let previous_log = std::env::var_os("OWNED_DOCKER_LOG");
        let previous_push_marker = std::env::var_os("OWNED_PUSH_MARKER");
        let mut entries = vec![dir.path().to_path_buf()];
        if let Some(path) = previous_path.as_ref() {
            entries.extend(std::env::split_paths(path));
        }
        unsafe {
            std::env::set_var("PATH", std::env::join_paths(entries).expect("owned PATH"));
            std::env::set_var("OWNED_DOCKER_LOG", &log);
            std::env::set_var("OWNED_PUSH_MARKER", &push_marker);
        }
        Self {
            _dir: dir,
            log,
            push_marker,
            previous_path,
            previous_log,
            previous_push_marker,
        }
    }
}

#[cfg(unix)]
impl Drop for OwnedDeployDocker {
    fn drop(&mut self) {
        unsafe {
            match self.previous_path.take() {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
            match self.previous_log.take() {
                Some(path) => std::env::set_var("OWNED_DOCKER_LOG", path),
                None => std::env::remove_var("OWNED_DOCKER_LOG"),
            }
            match self.previous_push_marker.take() {
                Some(path) => std::env::set_var("OWNED_PUSH_MARKER", path),
                None => std::env::remove_var("OWNED_PUSH_MARKER"),
            }
        }
    }
}

#[cfg(unix)]
fn prepare_failure_deploy(env: &Env, name: &str) -> std::path::PathBuf {
    let profile_dir = write_cloud_profile(env, name);
    std::fs::copy(
        env.root().join("system/signing_key.pem"),
        profile_dir.join("signing_key.pem"),
    )
    .expect("copy owned signing key");
    std::fs::write(
        profile_dir.join("secrets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "oauth_at_rest_pepper": "owned_failure_pepper_at_least_thirty_two_bytes",
            "database_url": "postgres://owned:owned@127.0.0.1:1/owned",
            "failure_marker": "same-owned-account"
        }))
        .expect("serialize failure deploy secrets"),
    )
    .expect("write failure deploy secrets");
    for path in [
        env.root().join("target/release"),
        env.root().join("storage/files"),
        env.root().join("web/dist"),
        env.root().join("services/web/templates"),
        profile_dir.join("docker"),
    ] {
        std::fs::create_dir_all(path).expect("create failure deploy artifact directory");
    }
    std::fs::write(
        env.root().join("target/release/systemprompt"),
        "owned binary",
    )
    .expect("write owned release artifact");
    systemprompt_cli::cloud::profile::templates::save_dockerfile(
        &profile_dir.join("docker/Dockerfile"),
        name,
        env.root(),
    )
    .expect("write validated profile Dockerfile");
    profile_dir
}

#[cfg(unix)]
async fn mount_failure_registry(server: &wiremock::MockServer) {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "owned-failure-bearer",
            "expires_in": 600
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/registry-token")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "registry": "registry.failure.invalid",
                "username": "owned-user",
                "token": "owned-registry-token",
                "repository": "systemprompt/owned",
                "tag": "failure"
            }
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[cfg(unix)]
async fn run_failure_deploy(name: &str) -> Result<(), String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cloud::execute(
            CloudCommands::Deploy {
                skip_push: true,
                profile: Some(name.to_owned()),
                check: false,
            },
            &json_ctx(),
        ),
    )
    .await
    .expect("owned failure deployment completes within its bound")
    .map_err(|error| format!("{error:#}"))
}

#[cfg(unix)]
#[tokio::test]
async fn docker_build_failure_stops_before_secret_or_deploy_requests() {
    let env = enter().await;
    env.server().reset().await;
    seed_tenants(env.root());
    let name = "deploy-build-failure";
    let profile_dir = prepare_failure_deploy(&env, name);
    let docker = OwnedDeployDocker::install(71);
    mount_failure_registry(env.server()).await;

    let failure = run_failure_deploy(name)
        .await
        .expect_err("a failed Docker build stops deployment");
    let message = failure;
    assert!(
        message.contains("Command failed: docker build --no-cache")
            && message.contains("registry.failure.invalid/systemprompt/owned:failure"),
        "the error must identify the failed owned image build: {message}"
    );
    let requests = env
        .server()
        .received_requests()
        .await
        .expect("recorded build-failure requests");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path().ends_with("/registry-token"))
            .count(),
        1
    );
    assert!(
        requests
            .iter()
            .all(|request| request.method.as_str() != "PUT"),
        "secret synchronization must not begin after a failed build"
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.url.path().ends_with("/deploy")),
        "deployment must not begin after a failed build"
    );
    let docker_calls = std::fs::read_to_string(&docker.log).expect("Docker calls");
    let expected_build = format!(
        "PWD={} ARGS=build --no-cache -f {} -t registry.failure.invalid/systemprompt/owned:failure .",
        env.root().display(),
        profile_dir.join("docker/Dockerfile").display()
    );
    assert_eq!(
        docker_calls
            .lines()
            .filter(|line| line.contains("ARGS=build"))
            .collect::<Vec<_>>(),
        [expected_build.as_str()]
    );

    remove_profile(&env, name);
}

#[cfg(unix)]
#[tokio::test]
async fn secret_sync_failure_blocks_deploy_then_retries_the_same_tenant_successfully() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    let env = enter().await;
    env.server().reset().await;
    seed_tenants(env.root());
    let name = "deploy-secret-retry";
    let _profile_dir = prepare_failure_deploy(&env, name);
    let docker = OwnedDeployDocker::install(0);
    mount_failure_registry(env.server()).await;
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(ResponseTemplate::new(503).set_body_string(format!(
            "secret synchronization rejected for tenant {TENANT_ID}"
        )))
        .expect(1)
        .mount(env.server())
        .await;

    let failure = run_failure_deploy(name)
        .await
        .expect_err("secret synchronization failure stops deployment");
    let message = failure;
    assert!(
        message.contains("503")
            && message.contains("secret synchronization")
            && message.contains(TENANT_ID),
        "the failure must retain the secret stage and selected tenant: {message}"
    );
    let failed_requests = env
        .server()
        .received_requests()
        .await
        .expect("recorded failed deployment requests");
    assert_eq!(
        failed_requests
            .iter()
            .filter(|request| request.method.as_str() == "PUT")
            .count(),
        1
    );
    assert!(
        failed_requests
            .iter()
            .all(|request| !request.url.path().ends_with("/deploy"))
    );

    env.server().reset().await;
    mount_failure_registry(env.server()).await;
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(ResponseTemplate::new(204))
        .expect(3)
        .mount(env.server())
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/deploy")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"status": "deploying", "app_url": "https://repaired.example.invalid"}
        })))
        .expect(1)
        .mount(env.server())
        .await;

    run_failure_deploy(name)
        .await
        .expect("repaired secret endpoint allows retry");
    let repaired_requests = env
        .server()
        .received_requests()
        .await
        .expect("recorded repaired deployment requests");
    let tenant_prefix = format!("/api/v1/tenants/{TENANT_ID}/");
    assert_eq!(
        repaired_requests
            .iter()
            .filter(|request| request.url.path().ends_with("/secrets"))
            .count(),
        3
    );
    assert_eq!(
        repaired_requests
            .iter()
            .filter(|request| request.url.path().ends_with("/deploy"))
            .count(),
        1
    );
    assert!(
        repaired_requests.iter().all(|request| {
            !request.url.path().starts_with("/api/v1/tenants/")
                || request.url.path().starts_with(&tenant_prefix)
        }),
        "every tenant-scoped retry request must retain the selected tenant"
    );
    let docker_calls = std::fs::read_to_string(&docker.log).expect("Docker calls");
    assert_eq!(
        docker_calls
            .lines()
            .filter(|line| line.contains("ARGS=build"))
            .count(),
        2,
        "the explicit retry reruns the owned image build before provisioning"
    );

    remove_profile(&env, name);
}
#[cfg(unix)]
#[tokio::test]
async fn deployment_logs_in_with_stdin_pushes_exact_image_then_provisions() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, Request, ResponseTemplate};

    let env = enter().await;
    env.server().reset().await;
    seed_tenants(env.root());
    let name = "deploy-owned-push";
    let profile_dir = prepare_failure_deploy(&env, name);
    let docker = OwnedDeployDocker::install_push();
    mount_failure_registry(env.server()).await;

    let push_observed_before_secrets = Arc::new(AtomicBool::new(true));
    let push_marker = docker.push_marker.clone();
    let ordering = Arc::clone(&push_observed_before_secrets);
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(move |_request: &Request| {
            ordering.fetch_and(push_marker.exists(), Ordering::SeqCst);
            ResponseTemplate::new(204)
        })
        .expect(3)
        .mount(env.server())
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/deploy")))
        .and(header("authorization", "Bearer owned-failure-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"status": "deploying", "app_url": "https://pushed.example.invalid"}
        })))
        .expect(1)
        .mount(env.server())
        .await;

    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cloud::execute(
            CloudCommands::Deploy {
                skip_push: false,
                profile: Some(name.to_owned()),
                check: false,
            },
            &json_ctx(),
        ),
    )
    .await
    .expect("owned pushed deployment completes within its bound")
    .expect("owned pushed deployment succeeds");

    assert!(
        push_observed_before_secrets.load(Ordering::SeqCst),
        "Docker push must finish before the first secret request"
    );
    let image = "registry.failure.invalid/systemprompt/owned:failure";
    let calls = std::fs::read_to_string(&docker.log).expect("Docker calls");
    let lines = calls.lines().collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            format!(
                "PWD={} ARGS=version --format {{{{.Server.Version}}}}",
                env.root().display()
            ),
            format!(
                "PWD={} ARGS=build --no-cache -f {} -t {image} .",
                env.root().display(),
                profile_dir.join("docker/Dockerfile").display()
            ),
            format!(
                "PWD={} ARGS=login registry.failure.invalid -u owned-user --password-stdin",
                env.root().display()
            ),
            "STDIN=owned-registry-token".to_owned(),
            format!("PWD={} ARGS=push {image}", env.root().display()),
        ]
    );
    let requests = env
        .server()
        .received_requests()
        .await
        .expect("recorded pushed deployment requests");
    let deploy = requests
        .iter()
        .find(|request| request.url.path().ends_with("/deploy"))
        .expect("deploy request after push");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&deploy.body).expect("deploy JSON"),
        serde_json::json!({"image": image})
    );

    remove_profile(&env, name);
}
