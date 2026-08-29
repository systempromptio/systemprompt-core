//! `plugins list` — the filters over the compiled extension registry.
//!
//! The registry is whatever is linked into this binary, so these assert
//! relative properties — a filter's result is a subset of the unfiltered one,
//! ordering holds, case does not matter — rather than naming extensions. An
//! assertion on specific ids would break whenever the linked set changed,
//! which says nothing about the filter.
//!
//! There is deliberately no test for the priority sort. Every linked
//! extension reports priority 0 or 100 and the registry already yields them
//! ascending, so removing `sort_by_key` changes nothing observable here — a
//! test for it passes whether or not the sort exists. Verified by removing the
//! sort and watching such a test stay green.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::plugins::list::{ListArgs, execute};
use systemprompt_cli::shared::CommandOutput;

fn args(filter: Option<&str>, capability: Option<&str>, kind: &str) -> ListArgs {
    ListArgs {
        filter: filter.map(str::to_owned),
        capability: capability.map(str::to_owned),
        r#type: kind.to_owned(),
    }
}

fn rows(out: &CommandOutput) -> Vec<serde_json::Value> {
    serde_json::to_value(out.artifact())
        .expect("serialise artifact")
        .get("items")
        .and_then(|i| i.as_array().cloned())
        .unwrap_or_default()
}

fn list(filter: Option<&str>, capability: Option<&str>, kind: &str) -> Vec<serde_json::Value> {
    rows(&execute(&args(filter, capability, kind), &CliConfig::new()))
}

fn ids(rows: &[serde_json::Value]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect()
}

fn compiled_ids() -> Vec<String> {
    ids(&list(None, None, "compiled"))
}

// Why: this suite's precondition. With nothing compiled in, every filter test
// below would pass by returning empty for the wrong reason.
#[test]
fn the_compiled_registry_is_not_empty_in_this_binary() {
    assert!(
        !compiled_ids().is_empty(),
        "no compiled extensions linked; the filter assertions would be vacuous"
    );
}

// Why: `--filter` is documented as a substring match, and it spans id and
// name. Anchoring it, or matching only the id, would silently hide
// extensions an operator searched for by name.
#[test]
fn a_filter_matches_a_substring_and_narrows_the_list() {
    let all = compiled_ids();
    let target = all
        .first()
        .expect("at least one compiled extension")
        .clone();
    let fragment: String = target.chars().take(3).collect();

    let filtered = compiled_ids_matching(&fragment);

    assert!(
        filtered.contains(&target),
        "filtering by a fragment of {target} should still return it: {filtered:?}"
    );
    assert!(
        filtered.len() <= all.len(),
        "a filter must never widen the list"
    );
}

fn compiled_ids_matching(fragment: &str) -> Vec<String> {
    ids(&list(Some(fragment), None, "compiled"))
}

// Why: an operator typing a name does not match the registry's casing. A
// case-sensitive filter returns nothing and reads as "no such extension".
#[test]
fn filtering_ignores_case() {
    let target = compiled_ids()
        .first()
        .expect("at least one compiled extension")
        .clone();
    let fragment: String = target.chars().take(3).collect();

    assert_eq!(
        compiled_ids_matching(&fragment.to_uppercase()),
        compiled_ids_matching(&fragment.to_lowercase()),
        "case must not change which extensions match"
    );
}

#[test]
fn a_filter_that_matches_nothing_returns_an_empty_list_rather_than_erroring() {
    assert!(compiled_ids_matching("no-such-extension-anywhere").is_empty());
}

// Why: `--type` selects which source is consulted. If `manifest` still
// collected compiled extensions the flag would be decorative.
#[test]
fn the_type_flag_selects_which_source_is_listed() {
    let compiled = list(None, None, "compiled");
    assert!(
        compiled.iter().all(|r| r["source"] == "compiled"),
        "the compiled listing must contain only compiled extensions"
    );

    let manifest = list(None, None, "manifest");
    assert!(
        manifest.iter().all(|r| r["source"] != "compiled"),
        "the manifest listing must not contain compiled extensions"
    );
}

// Why: `routes` is wired to return nothing for compiled extensions — their
// route count is reported as 0 regardless. Pinned so the emptiness reads as a
// decision rather than as a filter that quietly broke.
#[test]
fn the_routes_capability_matches_no_compiled_extension_by_design() {
    assert!(
        ids(&list(None, Some("routes"), "compiled")).is_empty(),
        "compiled extensions report no routes, so this filter selects none"
    );
}

// Why: a capability filter must actually consult the capability. Returning
// everything would advertise extensions that cannot do what was asked.
#[test]
fn a_capability_filter_returns_a_subset_of_the_unfiltered_list() {
    let all = compiled_ids();

    for capability in ["jobs", "schemas", "tools", "roles", "llm", "storage"] {
        let filtered = ids(&list(None, Some(capability), "compiled"));
        assert!(
            filtered.len() <= all.len(),
            "{capability} filter widened the list"
        );
        assert!(
            filtered.iter().all(|id| all.contains(id)),
            "{capability} filter returned an extension absent from the full list"
        );
    }
}
