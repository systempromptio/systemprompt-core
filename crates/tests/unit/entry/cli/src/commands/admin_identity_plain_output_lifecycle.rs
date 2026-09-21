//! Plain identity generation emits one internally consistent operator bundle.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use clap::Parser;
use systemprompt_cli::admin::identity::{self, IdentityCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_config::decode_seed;
use systemprompt_security::keys::RsaSigningKey;

const HELPER: &str = "commands::admin_identity_plain_output_lifecycle::plain_identity_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: IdentityCommands,
}

#[test]
#[ignore = "re-executed by plain_output_is_a_complete_cryptographically_consistent_bundle"]
fn plain_identity_helper() {
    let command = Harness::try_parse_from(["identity", "generate"])
        .expect("parse identity command")
        .command;
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Table),
        EnvOverrides::default(),
    );
    identity::execute(command, &context).expect("plain identity generation");
}

fn bounded_output(mut command: Command) -> Output {
    let mut stdout = tempfile::NamedTempFile::new().expect("stdout");
    let mut stderr = tempfile::NamedTempFile::new().expect("stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(s) = child.try_wait().expect("poll helper") {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait().expect("reap helper");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |f: &mut tempfile::NamedTempFile| {
        f.seek(SeekFrom::Start(0)).expect("rewind");
        let mut b = Vec::new();
        f.read_to_end(&mut b).expect("read");
        b
    };
    Output {
        status,
        stdout: read(&mut stdout),
        stderr: read(&mut stderr),
    }
}

#[test]
fn plain_output_is_a_complete_cryptographically_consistent_bundle() {
    let mut command = Command::new(std::env::current_exe().expect("unit binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(output.status.success(), "identity helper failed");
    let combined = format!(
        "{}\n{}",
        String::from_utf8(output.stdout).expect("UTF-8 stdout"),
        String::from_utf8(output.stderr).expect("UTF-8 stderr")
    );
    let value = |prefix: &str| {
        combined
            .lines()
            .find_map(|line| line.trim().strip_prefix(prefix))
            .map(str::to_owned)
            .unwrap_or_else(|| panic!("missing identity output field {prefix}"))
    };
    let pepper = value("oauth_at_rest_pepper=");
    let seed_encoded = value("manifest_signing_secret_seed=");
    let pem_encoded = value("signing_key_pem=");
    let kid = value("kid: ");
    assert_eq!(pepper.len(), 64);
    assert!(pepper.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    assert_eq!(decode_seed(&seed_encoded).expect("decode seed").len(), 32);
    let pem = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(&pem_encoded)
            .expect("decode PEM"),
    )
    .expect("UTF-8 PEM");
    let key = RsaSigningKey::from_pkcs8_pem(&pem).expect("parse signing key");
    assert_eq!(key.kid(), kid);
    assert_eq!(
        combined
            .lines()
            .filter(|line| line.contains("oauth_at_rest_pepper="))
            .count(),
        1
    );
    assert_eq!(
        combined
            .lines()
            .filter(|line| line.contains("manifest_signing_secret_seed="))
            .count(),
        1
    );
    assert_eq!(
        combined
            .lines()
            .filter(|line| line.contains("signing_key_pem="))
            .count(),
        1
    );
    assert_eq!(
        combined
            .lines()
            .filter(|line| line.contains("kid: "))
            .count(),
        1
    );
}
