//! The `cloud profile` interactive operation menu.
//!
//! With no subcommand the command loops on a picker until "Done" is chosen.
//! The loop, the unavailable-operation labels and the redirect that turns an
//! Edit/Delete choice into a List when no profile exists are reachable only
//! through a prompter, so they are driven here with a scripted one.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};

const LIST: &str = "0";
const EDIT: &str = "1";
const DELETE: &str = "2";
const DONE: &str = "3";

fn ctx(answers: &[&str]) -> CommandContext {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let env = EnvOverrides {
        profile: Some(boot.profile_path.to_string_lossy().to_string()),
        ..EnvOverrides::default()
    };
    CommandContext::new(
        CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
        env,
    )
    .with_prompter(Box::new(ScriptedPrompter::new(
        answers.iter().map(|s| (*s).to_owned()),
    )))
}

#[tokio::test]
async fn choosing_done_ends_the_menu_loop() {
    profile::execute(None, &ctx(&[DONE]))
        .await
        .expect("Done leaves the menu without running anything");
}

#[tokio::test]
async fn listing_returns_to_the_menu_rather_than_ending_it() {
    // Only one answer is scripted, so the prompter can satisfy the first
    // iteration and no more. The exhaustion error is the proof that the loop
    // came back for a second selection instead of exiting after the list.
    let err = profile::execute(None, &ctx(&[LIST]))
        .await
        .expect_err("listing must not terminate the menu");

    assert!(
        format!("{err:#}").contains("Scripted prompter exhausted"),
        "expected the menu to ask again after listing, got: {err:#}"
    );
}

#[tokio::test]
async fn listing_then_done_completes() {
    profile::execute(None, &ctx(&[LIST, DONE]))
        .await
        .expect("list then done");
}

#[tokio::test]
async fn editing_with_no_profiles_falls_back_to_listing() {
    profile::execute(None, &ctx(&[EDIT, DONE]))
        .await
        .expect("an Edit choice with no profiles lists instead of editing");
}

#[tokio::test]
async fn deleting_with_no_profiles_falls_back_to_listing() {
    profile::execute(None, &ctx(&[DELETE, DONE]))
        .await
        .expect("a Delete choice with no profiles lists instead of deleting");
}

#[tokio::test]
async fn a_selection_outside_the_menu_is_refused_by_the_prompter() {
    let err = profile::execute(None, &ctx(&["4"]))
        .await
        .expect_err("the menu has four entries");

    assert!(
        format!("{err:#}").contains("out of range"),
        "expected an out-of-range selection to be refused, got: {err:#}"
    );
}
