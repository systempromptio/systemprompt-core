use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::codex_cli::CODEX_CLI_HOST;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileRemoval};

struct Paths {
    managed: PathBuf,
    blocking_file: PathBuf,
}

struct GeneratedProfileGuard(PathBuf);

impl GeneratedProfileGuard {
    fn retain(path: &str) -> Self {
        Self(PathBuf::from(path))
    }
}

impl Drop for GeneratedProfileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn with_managed_path<R>(managed: impl Fn(&Path) -> PathBuf, body: impl FnOnce(&Paths) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let base = temp.path();
    let blocking_file = base.join("not-a-dir");
    fs::write(&blocking_file, "i am a file").expect("seed blocking file");
    let paths = Paths {
        managed: managed(base),
        blocking_file,
    };
    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(base.join("codex").display().to_string())),
        (
            "CODEX_SYSTEM_CONFIG",
            Some(paths.managed.display().to_string()),
        ),
    ];
    let out = temp_env::with_vars(vars, || body(&paths));
    drop(temp);
    out
}

fn inputs() -> ProfileGenInputs {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        host_token: HostToken::new("loopback-secret"),
        models: vec!["gpt-5".to_owned()],
        default_model: None,
        organization_uuid: Some("org-1234".to_owned()),
        headers,
        mcp_servers: Some(Vec::new()),
    }
}

#[test]
fn each_generated_profile_gets_its_own_file_and_uuids() {
    with_managed_path(
        |base| base.join("etc").join("config.toml"),
        |_paths| {
            let first = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("first generate");
            let second = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("second generate");
            assert_ne!(
                first.path, second.path,
                "two generations must not race onto one temp file"
            );
            assert!(
                first.bytes > 0
                    && first.bytes == fs::metadata(&first.path).expect("stat").len() as usize,
                "the reported size is the file's own: {} vs {}",
                first.bytes,
                first.path
            );
            assert!(
                !first.payload_uuid.is_empty() && !first.profile_uuid.is_empty(),
                "a profile carries both payload identifiers"
            );
            assert_ne!(
                first.payload_uuid, first.profile_uuid,
                "the payload and the profile are separately identified"
            );
        },
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn installing_then_removing_leaves_no_managed_config_behind() {
    with_managed_path(
        |base| base.join("etc").join("config.toml"),
        |paths| {
            let generated = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("generate");
            CODEX_CLI_HOST
                .install_profile(&generated.path)
                .expect("install into the sandbox");
            let merged = fs::read_to_string(&paths.managed).expect("managed config written");
            assert!(
                merged.contains("base_url = \"http://127.0.0.1:48217/v1\""),
                "the installed config points at the loopback proxy: {merged}"
            );

            let removal = CODEX_CLI_HOST.remove_profile().expect("remove");
            match removal {
                ProfileRemoval::Removed { path } => assert_eq!(
                    path.as_deref(),
                    Some(paths.managed.display().to_string().as_str()),
                    "the removal names the file it cleaned"
                ),
                other => panic!("expected Removed, got {other:?}"),
            }

            let after = CODEX_CLI_HOST.remove_profile().expect("second remove");
            assert!(
                matches!(after, ProfileRemoval::NothingToRemove),
                "a second removal has nothing left to do, got {after:?}"
            );
        },
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn a_removal_with_no_profile_installed_reports_nothing_to_remove() {
    with_managed_path(
        |base| base.join("etc").join("config.toml"),
        |paths| {
            assert!(!paths.managed.exists(), "the sandbox starts empty");
            let removal = CODEX_CLI_HOST.remove_profile().expect("remove");
            assert!(
                matches!(removal, ProfileRemoval::NothingToRemove),
                "got {removal:?}"
            );
        },
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn an_unbuildable_managed_directory_fails_with_the_elevation_hint() {
    with_managed_path(
        |base| base.join("not-a-dir").join("config.toml"),
        |paths| {
            let generated = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("generate");
            let err = CODEX_CLI_HOST
                .install_profile(&generated.path)
                .expect_err("a directory that cannot exist must not report a successful install");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
            let text = err.to_string();
            assert!(
                text.contains("Re-run as root"),
                "the error tells the operator how to finish it: {text}"
            );
            assert!(
                text.contains("bridge codex install"),
                "the hint names the command to re-run: {text}"
            );
            assert!(
                paths.blocking_file.is_file(),
                "the failed install left the blocking file alone"
            );
        },
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn a_managed_path_with_no_parent_is_refused_before_anything_is_written() {
    with_managed_path(
        |_base| PathBuf::from("/"),
        |_paths| {
            let generated = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("generate");
            let err = CODEX_CLI_HOST
                .install_profile(&generated.path)
                .expect_err("the filesystem root has no parent to install into");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
            assert!(
                err.to_string().contains("cannot resolve parent"),
                "got {err}"
            );
        },
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn a_malformed_managed_codex_target_keeps_the_generated_profile_for_repair_and_retry() {
    with_managed_path(
        |base| base.join("etc").join("config.toml"),
        |paths| {
            fs::create_dir_all(paths.managed.parent().expect("managed parent")).unwrap();
            let malformed = "[operator\n";
            fs::write(&paths.managed, malformed).unwrap();
            let generated = CODEX_CLI_HOST
                .generate_profile(&inputs())
                .expect("generate");
            let _generated_profile_guard = GeneratedProfileGuard::retain(&generated.path);
            let generated_bytes = fs::read(&generated.path).expect("generated profile bytes");

            CODEX_CLI_HOST
                .install_profile(&generated.path)
                .expect_err("malformed target must not merge");
            assert_eq!(
                fs::read(&generated.path).expect("retained generated profile"),
                generated_bytes,
                "a failed merge retains the exact generated source for retry"
            );
            assert_eq!(fs::read_to_string(&paths.managed).unwrap(), malformed);

            fs::write(&paths.managed, "operator_key = \"keep\"\n").unwrap();
            CODEX_CLI_HOST
                .install_profile(&generated.path)
                .expect("repaired target retries the generated profile");
            assert!(!std::path::Path::new(&generated.path).exists());
            let raw = fs::read_to_string(&paths.managed).expect("merged config");
            let merged: toml::Value =
                toml::from_str(&raw).expect("merged config remains valid TOML");
            assert_eq!(merged["operator_key"].as_str(), Some("keep"));
            assert_eq!(merged["model_provider"].as_str(), Some("systemprompt"));
            assert_eq!(
                merged["model_providers"]["systemprompt"]["base_url"].as_str(),
                Some("http://127.0.0.1:48217/v1")
            );
        },
    );
}
