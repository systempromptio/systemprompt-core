//! Tests for `admin keys generate`.
//!
//! The command mints an RSA-2048 PKCS#8 PEM at the requested path and refuses
//! to clobber an existing key without `--force`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::keys::{KeysCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: KeysCommands,
}

fn parse(args: &[&str]) -> KeysCommands {
    Harness::try_parse_from(std::iter::once("keys").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    crate::boot_ctx::new_with_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[tokio::test]
async fn generate_writes_a_pkcs8_pem_at_the_requested_path() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("signing_key.pem");

    execute(
        parse(&["generate", "--output", out.to_str().unwrap()]),
        &ctx(),
    )
    .await
    .unwrap();

    let pem = std::fs::read_to_string(&out).unwrap();
    assert!(pem.contains("BEGIN PRIVATE KEY"), "{pem}");
    assert!(pem.contains("END PRIVATE KEY"), "{pem}");
}

#[tokio::test]
async fn generate_refuses_to_overwrite_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("existing.pem");
    std::fs::write(&out, "sentinel").unwrap();

    let err = execute(
        parse(&["generate", "--output", out.to_str().unwrap()]),
        &ctx(),
    )
    .await
    .unwrap_err();

    assert!(format!("{err:#}").contains("Refusing to overwrite"));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "sentinel");
}

#[tokio::test]
async fn force_replaces_an_existing_key_with_a_fresh_one() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("rotate.pem");

    execute(
        parse(&["generate", "--output", out.to_str().unwrap()]),
        &ctx(),
    )
    .await
    .unwrap();
    let first = std::fs::read_to_string(&out).unwrap();

    execute(
        parse(&["generate", "--output", out.to_str().unwrap(), "--force"]),
        &ctx(),
    )
    .await
    .unwrap();
    let second = std::fs::read_to_string(&out).unwrap();

    assert_ne!(first, second, "a forced regeneration mints a new key");
    assert!(second.contains("BEGIN PRIVATE KEY"));
}

#[derive(Debug, Parser)]
struct AdminHarness {
    #[command(subcommand)]
    cmd: systemprompt_cli::admin::AdminCommands,
}

#[test]
fn rotate_is_classified_destructive_and_generate_preserving() {
    let admin = |args: &[&str]| {
        AdminHarness::try_parse_from(std::iter::once("admin").chain(args.iter().copied()))
            .unwrap()
            .cmd
            .data_impact()
    };
    assert_eq!(
        admin(&["keys", "rotate", "--overlap-secs", "60"]),
        systemprompt_cli::descriptor::DataImpact::Destructive
    );
    assert_eq!(
        admin(&["keys", "generate"]),
        systemprompt_cli::descriptor::DataImpact::Preserving
    );
}

#[tokio::test]
async fn rotate_refuses_a_profile_without_a_secrets_file_to_write() {
    let err = execute(parse(&["rotate"]), &ctx()).await.unwrap_err();
    let text = format!("{err:#}");
    assert!(
        text.contains("Failed to write the rotated signing key to the secrets file"),
        "{text}"
    );
    assert!(text.contains("No secrets configured"), "{text}");
}
