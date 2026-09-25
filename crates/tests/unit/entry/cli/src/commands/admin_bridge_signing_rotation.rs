#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use clap::Parser;
use systemprompt_cli::admin::bridge::{self, BridgeCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::profile_authoring::LocalProfileBuilder;
use systemprompt_config::ProfileBootstrap;

const HELPER: &str = "commands::admin_bridge_signing_rotation::signing_rotation_helper";
const ORIGINAL_SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: BridgeCommands,
}

fn rotate() -> BridgeCommands {
    Harness::try_parse_from(["bridge", "rotate-signing-key"])
        .expect("parse rotate signing key")
        .command
}

fn context() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[tokio::test]
#[ignore = "re-executed by rotation_changes_public_identity_and_persists_the_latest_seed"]
async fn signing_rotation_helper() {
    let fixture = tempfile::tempdir().expect("rotation fixture");
    let services = fixture.path().join("services");
    std::fs::create_dir_all(&services).unwrap();
    let secrets = fixture.path().join("secrets.json");
    std::fs::write(
        &secrets,
        serde_json::json!({
            "oauth_at_rest_pepper": "test-pepper-0123456789-abcdefghijkl",
            "database_url": "postgresql://unused.invalid/test",
            "manifest_signing_secret_seed": ORIGINAL_SEED,
            "encryption_master_key": "33".repeat(32)
        })
        .to_string(),
    )
    .unwrap();
    let mut profile = LocalProfileBuilder::new("rotation", &secrets, &services).build();
    profile.security.issuer = "https://rotation.test".to_owned();
    let profile_path = fixture.path().join("profile.yaml");
    std::fs::write(&profile_path, serde_yaml::to_string(&profile).unwrap()).unwrap();
    ProfileBootstrap::init_from_path(&profile_path).expect("initialize owned profile");

    println!("BEGIN_FIRST_ROTATION");
    bridge::execute(rotate(), &context())
        .await
        .expect("first signing rotation");
    println!("END_FIRST_ROTATION");
    println!("BEGIN_SECOND_ROTATION");
    bridge::execute(rotate(), &context())
        .await
        .expect("second signing rotation");
    println!("END_SECOND_ROTATION");

    let persisted: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&secrets).unwrap()).unwrap();
    let seed = persisted["manifest_signing_secret_seed"]
        .as_str()
        .expect("persisted signing seed");
    let decoded: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(seed)
        .expect("persisted seed base64")
        .try_into()
        .expect("persisted signing seed length");
    assert_ne!(seed, ORIGINAL_SEED);
    println!(
        "PERSISTED_PUBKEY={}",
        systemprompt_security::manifest_signing::pubkey_b64_from_seed(&decoded)
    );
    assert_eq!(
        persisted["oauth_at_rest_pepper"], "test-pepper-0123456789-abcdefghijkl",
        "rotation must preserve unrelated secrets"
    );
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().unwrap();
    let stderr = tempfile::NamedTempFile::new().unwrap();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn signing rotation helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("poll rotation helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap rotation helper");
            let output = Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
            panic!(
                "rotation helper timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn marked<'a>(stdout: &'a str, marker: &str) -> &'a str {
    stdout
        .split_once(&format!("BEGIN_{marker}"))
        .and_then(|(_, tail)| tail.split_once(&format!("END_{marker}")))
        .map(|(json, _)| json.trim())
        .unwrap_or_else(|| panic!("missing rotation marker {marker}: {stdout}"))
}

fn pubkey(artifact: &serde_json::Value) -> String {
    artifact["sections"]
        .as_array()
        .expect("rotation sections")
        .iter()
        .find(|section| section["heading"] == "pubkey_b64")
        .expect("public key section")["content"]
        .as_str()
        .expect("public key string")
        .to_owned()
}

#[test]
fn rotation_changes_public_identity_and_persists_the_latest_seed() {
    let mut command = Command::new(std::env::current_exe().expect("unit test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "rotation helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("rotation output UTF-8");
    let first: serde_json::Value =
        serde_json::from_str(marked(&stdout, "FIRST_ROTATION")).expect("first rotation JSON");
    let second: serde_json::Value =
        serde_json::from_str(marked(&stdout, "SECOND_ROTATION")).expect("second rotation JSON");
    assert_eq!(first["title"], "Bridge Signing Key Rotated", "{first}");
    assert_eq!(second["title"], "Bridge Signing Key Rotated", "{second}");
    let first_key = pubkey(&first);
    let second_key = pubkey(&second);
    assert_ne!(
        first_key, second_key,
        "each rotation changes bridge identity"
    );
    let persisted_key = stdout
        .lines()
        .find_map(|line| line.strip_prefix("PERSISTED_PUBKEY="))
        .expect("persisted public key marker");
    assert_eq!(
        persisted_key, second_key,
        "the latest reported bridge identity must derive from the seed that survives on disk"
    );
    for key in [first_key, second_key] {
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(key)
                .expect("public key base64")
                .len(),
            32
        );
    }
}
