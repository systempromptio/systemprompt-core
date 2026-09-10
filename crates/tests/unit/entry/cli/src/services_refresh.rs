//! Tests for `core services refresh` exit-code mapping and state diffing.
//!
//! The exit code is a contract with a supervisor script, so the mapping and
//! the "did anything change" decision are exercised directly rather than
//! through the process.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::core::services::refresh::{
    RefreshOutcome, diff_states, exit_code_for, outcome_for,
};
use systemprompt_models::services::bundle::{BundleSourceState, ServicesBundleState};

fn state(entries: &[(&str, &str)]) -> ServicesBundleState {
    let sources: BTreeMap<String, BundleSourceState> = entries
        .iter()
        .map(|(name, digest)| {
            (
                (*name).to_owned(),
                BundleSourceState {
                    digest: (*digest).to_owned(),
                    version: "1.0.0".to_owned(),
                    content_hash: format!("hash-{digest}"),
                    fetched_at: Utc::now(),
                },
            )
        })
        .collect();
    ServicesBundleState {
        composed_hash: "composed".to_owned(),
        last_reconciled_hash: None,
        sources,
    }
}

#[test]
fn exit_codes_distinguish_a_swap_from_a_no_op() {
    assert_eq!(exit_code_for(RefreshOutcome::Unchanged), 0);
    assert_eq!(exit_code_for(RefreshOutcome::Changed), 3);
}

#[test]
fn identical_state_reports_no_change() {
    let before = state(&[("base", "sha256:aaa")]);
    let rows = diff_states(&before, &before);
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].changed);
    assert_eq!(rows[0].previous_digest, "sha256:aaa");
    assert_eq!(outcome_for(&rows), RefreshOutcome::Unchanged);
}

#[test]
fn a_new_digest_reports_a_change_and_keeps_the_previous_one() {
    let before = state(&[("base", "sha256:aaa")]);
    let after = state(&[("base", "sha256:bbb")]);
    let rows = diff_states(&before, &after);
    assert!(rows[0].changed);
    assert_eq!(rows[0].previous_digest, "sha256:aaa");
    assert_eq!(rows[0].new_digest, "sha256:bbb");
    assert_eq!(outcome_for(&rows), RefreshOutcome::Changed);
}

#[test]
fn a_first_ever_fetch_reports_a_change_with_an_empty_previous_digest() {
    let before = ServicesBundleState::default();
    let after = state(&[("base", "sha256:aaa")]);
    let rows = diff_states(&before, &after);
    assert!(rows[0].changed);
    assert!(rows[0].previous_digest.is_empty());
    assert_eq!(outcome_for(&rows), RefreshOutcome::Changed);
}

#[test]
fn one_changed_source_among_several_is_enough() {
    let before = state(&[("base", "sha256:aaa"), ("sales", "sha256:ccc")]);
    let after = state(&[("base", "sha256:aaa"), ("sales", "sha256:ddd")]);
    let rows = diff_states(&before, &after);
    assert_eq!(rows.iter().filter(|r| r.changed).count(), 1);
    assert_eq!(outcome_for(&rows), RefreshOutcome::Changed);
}
