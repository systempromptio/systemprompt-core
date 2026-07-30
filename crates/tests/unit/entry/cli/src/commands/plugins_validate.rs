//! Tests for `plugins validate`.
//!
//! The command walks the compile-time extension registry and reports dependency
//! errors plus config/asset warnings. Its second return value is the process
//! exit signal, so it must track `output.valid` exactly — a mismatch would make
//! a failing validation exit zero.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::plugins::validate::{ValidateArgs, execute};

fn args(verbose: bool) -> ValidateArgs {
    ValidateArgs { verbose }
}

fn card(output: &systemprompt_cli::shared::CommandOutput) -> serde_json::Value {
    serde_json::to_value(output.artifact()).unwrap()
}

fn sections(value: &serde_json::Value) -> Vec<(String, serde_json::Value)> {
    value["sections"]
        .as_array()
        .expect("a presentation card has sections")
        .iter()
        .map(|s| {
            (
                s["heading"].as_str().unwrap_or_default().to_owned(),
                s["content"].clone(),
            )
        })
        .collect()
}

#[test]
fn reports_a_titled_validation_card() {
    let (output, _valid) = execute(&args(false), &CliConfig::new());
    let value = card(&output);

    assert_eq!(value["title"], "Extension Validation");
    let headings: Vec<String> = sections(&value).into_iter().map(|(h, _)| h).collect();
    for expected in ["valid", "extension_count", "errors", "warnings"] {
        assert!(
            headings.iter().any(|h| h == expected),
            "the card should report `{expected}`, got {headings:?}"
        );
    }
}

#[test]
fn exit_flag_tracks_the_reported_valid_field() {
    for verbose in [false, true] {
        let (output, valid) = execute(&args(verbose), &CliConfig::new());
        let value = card(&output);
        let reported = sections(&value)
            .into_iter()
            .find(|(h, _)| h == "valid")
            .map(|(_, c)| c)
            .expect("valid section");

        assert_eq!(
            serde_json::Value::Bool(valid),
            reported,
            "the exit flag and the rendered `valid` field must agree (verbose={verbose})"
        );
    }
}

#[test]
fn the_linked_registry_validates_cleanly() {
    // The registry here is whatever this test binary links, so a dependency
    // error would mean a genuinely broken extension declaration in-tree.
    let (_output, valid) = execute(&args(false), &CliConfig::new());
    assert!(
        valid,
        "the in-tree extension set should have no dependency errors"
    );
}

#[test]
fn extension_count_is_stable_across_verbosity() {
    let quiet = card(&execute(&args(false), &CliConfig::new()).0);
    let verbose = card(&execute(&args(true), &CliConfig::new()).0);

    let count = |v: &serde_json::Value| {
        sections(v)
            .into_iter()
            .find(|(h, _)| h == "extension_count")
            .map(|(_, c)| c)
            .expect("extension_count section")
    };

    assert_eq!(
        count(&quiet),
        count(&verbose),
        "verbosity changes which extensions are inspected, not how many exist"
    );
}
