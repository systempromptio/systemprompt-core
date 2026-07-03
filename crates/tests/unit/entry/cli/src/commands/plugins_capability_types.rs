//! Pure-type tests for the plugin capability summary rendering.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::plugins::types::CapabilitySummary;

#[test]
fn empty_summary_renders_none() {
    let summary = CapabilitySummary::default();
    assert_eq!(summary.summary_string(), "none");
}

#[test]
fn singular_labels_used_for_count_of_one() {
    let summary = CapabilitySummary {
        jobs: 1,
        templates: 1,
        schemas: 1,
        routes: 1,
        tools: 1,
        roles: 1,
        llm_providers: 1,
        storage_paths: 0,
    };
    let rendered = summary.summary_string();
    assert_eq!(
        rendered,
        "1 job, 1 template, 1 schema, 1 route, 1 tool, 1 role, 1 LLM"
    );
}

#[test]
fn plural_labels_used_for_counts_above_one() {
    let summary = CapabilitySummary {
        jobs: 2,
        templates: 3,
        schemas: 4,
        routes: 5,
        tools: 6,
        roles: 7,
        llm_providers: 8,
        storage_paths: 9,
    };
    let rendered = summary.summary_string();
    assert_eq!(
        rendered,
        "2 jobs, 3 templates, 4 schemas, 5 routes, 6 tools, 7 roles, 8 LLMs"
    );
}

#[test]
fn zero_valued_categories_are_omitted() {
    let summary = CapabilitySummary {
        jobs: 3,
        templates: 0,
        schemas: 0,
        routes: 0,
        tools: 2,
        roles: 0,
        llm_providers: 0,
        storage_paths: 0,
    };
    assert_eq!(summary.summary_string(), "3 jobs, 2 tools");
}

#[test]
fn summary_round_trips_through_json() {
    let summary = CapabilitySummary {
        jobs: 1,
        templates: 2,
        schemas: 3,
        routes: 4,
        tools: 5,
        roles: 6,
        llm_providers: 7,
        storage_paths: 8,
    };
    let json = serde_json::to_string(&summary).unwrap();
    let back: CapabilitySummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back.jobs, 1);
    assert_eq!(back.storage_paths, 8);
    assert_eq!(back.summary_string(), summary.summary_string());
}
