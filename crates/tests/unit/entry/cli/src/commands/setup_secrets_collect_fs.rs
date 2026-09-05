//! Secrets collection during `admin setup`.
//!
//! Both collectors mint a fresh identity before they do anything else, so each
//! test here pays one RSA keypair generation; the suite is deliberately kept to
//! the three arms that differ — flags supplied, a provider chosen at the
//! prompt, and nothing supplied at all — rather than one test per permutation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::admin::setup::secrets::{collect_interactive, collect_non_interactive};
use systemprompt_cli::interactive::ScriptedPrompter;

fn args() -> SetupArgs {
    SetupArgs {
        environment: None,
        docker: false,
        db_host: "127.0.0.1".to_owned(),
        db_port: 5432,
        port_offset: 0,
        db_user: None,
        db_password: None,
        db_name: None,
        gemini_key: None,
        anthropic_key: None,
        openai_key: None,
        github_token: None,
        default_provider: None,
        admin_email: None,
        migrate: false,
        no_migrate: false,
        dry_run: false,
        yes: false,
        force: false,
    }
}

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn config() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

#[test]
fn keys_passed_as_flags_are_taken_without_prompting_and_name_the_primary() {
    let mut args = args();
    args.anthropic_key = Some("sk-ant-fixture".to_owned());

    // Nothing is scripted: reaching a prompt at all would surface as an error.
    let (secrets, primary) = collect_interactive(&args, &scripted(&[]), "dev", &config())
        .expect("supplied keys skip the whole prompt sequence");

    assert_eq!(secrets.anthropic.as_deref(), Some("sk-ant-fixture"));
    assert_eq!(
        primary.map(|p| p.as_str().to_owned()).as_deref(),
        Some("anthropic"),
        "the only configured provider must become the default"
    );
    assert!(
        !secrets.oauth_at_rest_pepper.is_empty()
            && secrets.signing_key_pem.is_some()
            && secrets.manifest_signing_secret_seed.is_some(),
        "an identity is minted even when the keys came from flags"
    );
}

#[test]
fn a_provider_chosen_at_the_prompt_supplies_its_key_and_becomes_the_default() {
    let (secrets, primary) =
        collect_interactive(&args(), &scripted(&["1", "sk-ant-typed"]), "dev", &config())
            .expect("choosing Anthropic and entering a key completes setup");

    assert_eq!(secrets.anthropic.as_deref(), Some("sk-ant-typed"));
    assert!(
        secrets.gemini.is_none() && secrets.openai.is_none(),
        "only the chosen provider is configured"
    );
    assert_eq!(
        primary.map(|p| p.as_str().to_owned()).as_deref(),
        Some("anthropic")
    );
}

// Why: this is the gate that stops a profile being written with no way to
// reach a model at all. Skipping every key at the multi-key prompt is the
// cheapest way for an operator to arrive there.
#[test]
fn skipping_every_key_at_the_multi_key_prompt_is_refused() {
    let err = collect_interactive(&args(), &scripted(&["3", "", "", "", ""]), "dev", &config())
        .expect_err("a setup with no AI provider must not be accepted");

    assert!(
        format!("{err:#}").contains("At least one AI provider API key is required"),
        "the refusal must say which key is missing, got: {err:#}"
    );
}

#[test]
fn the_non_interactive_collector_refuses_the_same_way_and_accepts_a_flag() {
    let err = collect_non_interactive(&args(), &config())
        .expect_err("an unattended setup with no key cannot proceed");
    assert!(
        format!("{err:#}").contains("At least one AI provider API key is required"),
        "the unattended refusal must name the missing key, got: {err:#}"
    );

    let mut args = args();
    args.openai_key = Some("sk-openai-fixture".to_owned());
    let (secrets, primary) =
        collect_non_interactive(&args, &config()).expect("a supplied key is enough");
    assert_eq!(secrets.openai.as_deref(), Some("sk-openai-fixture"));
    assert_eq!(
        primary.map(|p| p.as_str().to_owned()).as_deref(),
        Some("openai")
    );
}
