//! Front matter of agent, skill and command files: repair of hand-written
//! values without damaging structured ones, and refusal of broken blocks.

use serde_json::{Value, json};
use systemprompt_engine::config::Loaded;

use crate::support::Tree;

fn agent_data(front: &str) -> (Loaded, Option<serde_json::Map<String, Value>>) {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/probe.md",
        &format!("---\n{front}---\nBody text.\n"),
    );
    let loaded = tree.load();
    let data = loaded.agent("probe").map(|agent| {
        let mut data = agent.extra.clone();
        if let Some(description) = &agent.description {
            data.insert("description".to_owned(), json!(description));
        }
        data
    });
    (loaded, data)
}

fn loads(front: &str) -> serde_json::Map<String, Value> {
    let (loaded, data) = agent_data(front);
    data.unwrap_or_else(|| panic!("not loaded: {:?}", loaded.problems))
}

#[test]
fn flow_collections_survive_the_repair_of_another_field() {
    let data = loads("description: Use when: reviewing\nlist: [a, b]\nmap: {k: v}\n");
    assert_eq!(data["description"], "Use when: reviewing");
    assert_eq!(data["list"], json!(["a", "b"]));
    assert_eq!(data["map"], json!({"k": "v"}));
}

#[test]
fn nested_flow_collections_quoted_delimiters_and_comments_survive_repair() {
    let data = loads(
        "description: Note: careful\nnested: {a: [1, 2], b: {c: d}}\nquoted: \"x: y, z\"\nplain: value # a comment\n",
    );
    assert_eq!(data["description"], "Note: careful");
    assert_eq!(data["nested"], json!({"a": [1, 2], "b": {"c": "d"}}));
    assert_eq!(data["quoted"], "x: y, z");
    assert_eq!(data["plain"], "value");
}

#[test]
fn multi_line_flow_collections_survive_repair_of_fields_around_them() {
    let data = loads("description: Before: one\nitems: [\n  a,\n  b\n]\nafter: Then: two\n");
    assert_eq!(data["description"], "Before: one");
    assert_eq!(data["items"], json!(["a", "b"]));
    assert_eq!(data["after"], "Then: two");
}

#[test]
fn crlf_line_endings_do_not_stop_repair() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/probe.md",
        "---\r\ndescription: Use when: x\r\nlist: [a, b]\r\n---\r\nBody\r\n",
    );
    let loaded = tree.load();
    let agent = loaded
        .agent("probe")
        .unwrap_or_else(|| panic!("{:?}", loaded.problems));
    assert_eq!(agent.description.as_deref(), Some("Use when: x"));
    assert_eq!(agent.extra["list"], json!(["a", "b"]));
}

#[test]
fn block_scalar_headers_with_a_commented_colon_are_kept() {
    let data = loads("description: |  # note: kept\n  line one\n  line two\nother: Use when: x\n");
    assert_eq!(data["description"], "line one\nline two\n");
    assert_eq!(data["other"], "Use when: x");
}

#[test]
#[ignore = "parity ledger: a value opening with a bracketed tag is not repaired"]
fn a_value_starting_with_a_bracketed_tag_is_recovered() {
    let data = loads("description: [WIP] Use when: drafting\n");
    assert_eq!(data["description"], "[WIP] Use when: drafting");
}

#[test]
#[ignore = "parity ledger: a value opening with a braced tag or carrying an apostrophe is not repaired"]
fn leading_braced_tags_with_colons_or_apostrophes_are_recovered() {
    let data = loads("description: {beta} it's: here\n");
    assert_eq!(data["description"], "{beta} it's: here");
}

#[test]
fn unclosed_or_mismatched_flow_collections_still_fail_and_keep_failing() {
    for front in ["list: [a, b\n", "map: {a: [1}\n"] {
        for _ in 0..2 {
            let (loaded, data) = agent_data(front);
            assert!(data.is_none(), "{front}");
            assert_eq!(loaded.problems.len(), 1, "{front}: {:?}", loaded.problems);
        }
    }
}

#[test]
fn the_same_repairable_content_parses_the_same_way_every_time() {
    let front = "description: Use when: twice\n";
    let first = loads(front);
    let _ = agent_data("list: [broken\n");
    assert_eq!(loads(front), first);
}

#[test]
fn front_matter_is_split_off_only_when_it_is_a_well_formed_leading_block() {
    let tree = Tree::new();
    tree.project(
        ".harness/commands/late.md",
        "Intro\n---\ndescription: not front matter\n---\nBody\n",
    );
    tree.project(
        ".harness/commands/lead.md",
        "---\ndescription: real\n---\nBody\n",
    );
    let loaded = tree.load();
    assert_eq!(loaded.commands["late"].description, None);
    assert!(loaded.commands["late"].template.starts_with("Intro"));
    assert_eq!(loaded.commands["lead"].description.as_deref(), Some("real"));
    assert_eq!(loaded.commands["lead"].template, "Body");
}

#[test]
fn quoted_block_folded_and_nested_values_read_as_written_with_hashes_as_text() {
    let data = loads(
        "single: 'it''s # not a comment'\ndouble: \"a # b\"\nblock: |\n  keep # this\n  too\nfolded: >\n  one\n  two\nnested:\n  key: value\n  list:\n    - x\n    - y\n",
    );
    assert_eq!(data["single"], "it's # not a comment");
    assert_eq!(data["double"], "a # b");
    assert_eq!(data["block"], "keep # this\ntoo\n");
    assert_eq!(data["folded"], "one two\n");
    assert_eq!(data["nested"], json!({"key": "value", "list": ["x", "y"]}));
}
