//! Tests for `admin identity generate` and the identity bundle it emits.
//!
//! The three values must decode in the encodings the secrets loader expects
//! and must differ between calls, so two nodes never share a bundle by
//! accident of the generator.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use base64::Engine;
use clap::Parser;
use systemprompt_cli::admin::identity::{IdentityCommands, execute};
use systemprompt_cli::shared::generate_identity;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_config::decode_seed;
use systemprompt_security::keys::RsaSigningKey;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: IdentityCommands,
}

fn parse(args: &[&str]) -> IdentityCommands {
    Harness::try_parse_from(std::iter::once("identity").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[test]
fn bundle_decodes_in_the_loader_encodings() {
    let bundle = generate_identity().unwrap();

    assert_eq!(bundle.oauth_at_rest_pepper.len(), 64);
    let seed = decode_seed(&bundle.manifest_signing_secret_seed).unwrap();
    assert_eq!(seed.len(), 32);
    let pem_bytes = base64::engine::general_purpose::STANDARD
        .decode(&bundle.signing_key_pem)
        .unwrap();
    let pem = String::from_utf8(pem_bytes).unwrap();
    let key = RsaSigningKey::from_pkcs8_pem(&pem).unwrap();
    assert_eq!(key.kid(), bundle.signing_kid);
}

#[test]
fn every_bundle_is_distinct() {
    let a = generate_identity().unwrap();
    let b = generate_identity().unwrap();
    assert_ne!(a.oauth_at_rest_pepper, b.oauth_at_rest_pepper);
    assert_ne!(
        a.manifest_signing_secret_seed,
        b.manifest_signing_secret_seed
    );
    assert_ne!(a.signing_key_pem, b.signing_key_pem);
}

#[test]
fn generate_command_runs_without_writing_files() {
    let tmp = tempfile::tempdir().unwrap();
    let before: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
    execute(parse(&["generate", "--json"]), &ctx()).unwrap();
    execute(parse(&["generate"]), &ctx()).unwrap();
    let after: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
    assert_eq!(before.len(), after.len());
}
