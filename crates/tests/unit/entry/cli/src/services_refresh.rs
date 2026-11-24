//! Tests for `core services refresh` exit-code mapping and state diffing.
//!
//! The exit code is a contract with a supervisor script, so the mapping and
//! the "did anything change" decision are exercised directly rather than
//! through the process.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::core::services::refresh::{
    RefreshOutcome, diff_states, exit_code_for, outcome_for, outcome_with_pending,
    refused_or_fallback,
};
use systemprompt_identifiers::ContentHash;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_manifest::services::bundle::{BundleSourceState, ServicesBundleState};
use systemprompt_manifest::services::root::ServicesProvenance;

fn state(entries: &[(&str, &str)]) -> ServicesBundleState {
    let sources: BTreeMap<String, BundleSourceState> = entries
        .iter()
        .map(|(name, digest)| {
            (
                (*name).to_owned(),
                BundleSourceState {
                    digest: (*digest).to_owned(),
                    version: "1.0.0".to_owned(),
                    content_hash: ContentHash::new(format!(
                        "{:0>64.64}",
                        digest
                            .bytes()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    )),
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
fn a_fallback_the_cache_could_not_record_is_still_an_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());

    for provenance in [
        ServicesProvenance::LastGood {
            composed_hash: "composed".to_owned(),
            error: "Permission denied".to_owned(),
        },
        ServicesProvenance::BundledFallback {
            error: "Permission denied".to_owned(),
        },
    ] {
        let error = refused_or_fallback(&provenance, &cache).expect_err("a fallback");
        assert!(error.to_string().contains("Permission denied"), "{error}");
    }
}

#[test]
fn an_unreadable_refusal_record_is_an_error_and_a_bundled_tree_has_none() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    let fetched = ServicesProvenance::Fetched {
        composed_hash: "composed".to_owned(),
        versions: BTreeMap::new(),
    };
    assert!(
        refused_or_fallback(&fetched, &cache)
            .expect("clean")
            .is_empty()
    );

    std::fs::write(root.path().join("refusals.json"), "{").expect("write");

    assert!(refused_or_fallback(&fetched, &cache).is_err());
    assert!(
        refused_or_fallback(&ServicesProvenance::Bundled, &cache)
            .expect("no sources")
            .is_empty()
    );
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

#[test]
fn an_unchanged_source_with_an_unfinished_reconcile_still_reports_a_change() {
    let same = state(&[("base", "sha256:aaa")]);
    let rows = diff_states(&same, &same);

    assert_eq!(
        outcome_with_pending(&rows, false),
        RefreshOutcome::Unchanged
    );
    assert_eq!(
        outcome_with_pending(&rows, true),
        RefreshOutcome::Changed,
        "a reconcile that failed after the state was written is retried, not forgotten"
    );
}
