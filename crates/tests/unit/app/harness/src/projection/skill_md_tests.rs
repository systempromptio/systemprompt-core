use std::fs;

use serde_json::{Value, json};
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_harness::projection::{BundleFiles, SKILLS_DIR, SKILLS_LEDGER};

use crate::projection::fixtures::{self, projected, skill};

fn rendered(entry: &Value) -> String {
    let manifest = fixtures::manifest(json!({ "skills": [entry] }));
    projected(&manifest, &BundleFiles::new())
        .skills
        .and_then(|set| set.files.into_iter().next())
        .map(|file| file.markdown)
        .unwrap_or_default()
}

fn ledger_version(entries: &[Value]) -> String {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let manifest = fixtures::manifest(json!({ "skills": entries }));
    applier
        .apply(&projected(&manifest, &BundleFiles::new()))
        .unwrap();
    let text =
        fs::read_to_string(applier.config_dir().join(SKILLS_DIR).join(SKILLS_LEDGER)).unwrap();
    let ledger: Value = serde_json::from_str(&text).unwrap();
    ledger["version"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn a_plain_body_gets_name_description_and_a_final_newline() {
    let text = rendered(&skill(
        "agent_farm_runs",
        json!({ "instructions": "# Agent farm runs\n\nUse when someone asks.\n" }),
    ));
    assert_eq!(
        text,
        "---\nname: agent-farm-runs\ndescription: Plain description.\n---\n\n# Agent farm runs\n\nUse when someone asks.\n"
    );
    let text = rendered(&skill("x", json!({ "instructions": "No final newline" })));
    assert_eq!(
        text,
        "---\nname: x\ndescription: Plain description.\n---\n\nNo final newline\n"
    );
}

#[test]
fn descriptions_are_quoted_only_when_yaml_would_misread_them() {
    let line = |text: &str| {
        let markdown = rendered(&skill("x", json!({ "description": text })));
        markdown.lines().nth(2).unwrap_or_default().to_owned()
    };
    assert_eq!(
        line("Operate the farm (\"la farm\"). Use it ONLY when: it is named."),
        "description: \"Operate the farm (\\\"la farm\\\"). Use it ONLY when: it is named.\""
    );
    assert_eq!(line("Use # sparingly"), "description: \"Use # sparingly\"");
    assert_eq!(
        line("- starts like a list"),
        "description: \"- starts like a list\""
    );
    assert_eq!(
        line("Route Odoo questions - DynBrain for how-to"),
        "description: Route Odoo questions - DynBrain for how-to"
    );
    assert_eq!(line(""), "description: \"\"");
    let missing = fixtures::manifest_body(json!({
        "skills": [{ "id": "x", "name": "x", "instructions": "b" }]
    }));
    let refused = Manifest::parse(&missing.to_string()).unwrap_err();
    assert!(refused.to_string().contains("description"), "{refused}");
}

#[test]
fn own_front_matter_keeps_its_lines_but_the_name_is_the_directory() {
    let text = rendered(&skill(
        "my_skill",
        json!({ "instructions": "\n---\nname: Display Name\ndescription: Own\n\nlicense: MIT\n  name: indented\n---\n# Body" }),
    ));
    assert_eq!(
        text,
        "---\nname: my-skill\ndescription: Own\nlicense: MIT\n---\n# Body\n"
    );
}

#[test]
fn author_keys_follow_sorted_without_the_platform_keys() {
    let text = rendered(&skill(
        "x",
        json!({ "frontmatter": {
            "tags": ["a"],
            "name": "ignored",
            "title": "ignored",
            "hosts": ["harness"],
            "metadata": { "zeta": 1, "alpha": ["one", { "b": true, "a": null }] },
            "license": "MIT",
            "allowed-tools": "Bash(git:*)",
            "empty": [],
            "flagged": "true",
            "number": "42"
        } }),
    ));
    let expected = [
        "---",
        "name: x",
        "description: Plain description.",
        "allowed-tools: Bash(git:*)",
        "empty: []",
        "flagged: 'true'",
        "license: MIT",
        "metadata:",
        "  alpha:",
        "  - one",
        "  - a: null",
        "    b: true",
        "  zeta: 1",
        "number: '42'",
        "---",
        "",
        "# Body",
        "",
        "Do it.",
        "",
    ]
    .join("\n");
    assert_eq!(text, expected);
    let only_owned = rendered(&skill(
        "x",
        json!({ "frontmatter": { "tags": ["a"], "hosts": [] } }),
    ));
    assert_eq!(
        only_owned,
        "---\nname: x\ndescription: Plain description.\n---\n\n# Body\n\nDo it.\n"
    );
}

#[test]
fn the_version_hashes_every_directory_and_file() {
    let both = ledger_version(&[skill("a_one", json!({})), skill("b_two", json!({}))]);
    assert_eq!(both, "a55bcf1f4e8ae59a");
    let one = ledger_version(&[skill("a_one", json!({}))]);
    assert_ne!(one, both);
    let none = ledger_version(&[skill("other", json!({ "hosts": ["claude-code"] }))]);
    assert_eq!(none.len(), 16);
    assert_ne!(none, one);
}
