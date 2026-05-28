//! Locks the skill-catalog invariant for the bridge marketplace and the
//! A2A agent card.
//!
//! Two cross-cutting guarantees Phase A of the skill-catalog refactor must
//! keep in place forever:
//!
//! 1. The bridge manifest's `skills[]` list is sourced exclusively from
//!    `services/skills/<id>/config.yaml`. Agent YAML `card.skills[]` arrays
//!    must NOT leak into the manifest, even when they reference skill ids
//!    that do not exist on disk.
//! 2. An agent's `metadata.skills: [id, ...]` is the only authored signal
//!    that drives the manifest's `AgentEntry.skills` field. Authored
//!    `card.skills` (now deprecated) must be ignored by the bridge manifest.
//!
//! Both tests exercise the manifest loaders directly against a synthetic
//! `services/` tree on disk so they are free of the shared `OnceLock`
//! bootstrap fixture and can drive arbitrary catalog state.

use std::fs;

use systemprompt_api::routes::gateway::bridge_manifest::agents::load_agents;
use systemprompt_api::routes::gateway::bridge_manifest::scope::scope_to_marketplace;
use systemprompt_api::routes::gateway::bridge_manifest::skills::load_skills;
use systemprompt_identifiers::SkillId;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentSkillConfig, CapabilitiesConfig,
    OAuthConfig, ServicesConfig,
};
use tempfile::TempDir;

const SKILL_FOO_CONFIG: &str = r#"
id: foo
name: Foo
description: A real on-disk skill.
enabled: true
tags: []
"#;

const SKILL_FOO_INSTRUCTIONS: &str = "Foo skill instructions.\n";

fn write_skill_foo(services_root: &std::path::Path) {
    let skills_dir = services_root.join("skills");
    fs::create_dir_all(skills_dir.join("foo")).expect("create skills/foo");
    fs::write(skills_dir.join("foo/config.yaml"), SKILL_FOO_CONFIG).expect("write skill config");
    fs::write(skills_dir.join("foo/SKILL.md"), SKILL_FOO_INSTRUCTIONS).expect("write skill md");
}

fn empty_card() -> AgentCardConfig {
    AgentCardConfig {
        protocol_version: "1.0".to_owned(),
        name: None,
        display_name: "Phantom Agent".to_owned(),
        description: "An agent.".to_owned(),
        version: "1.0.0".to_owned(),
        preferred_transport: "JSONRPC".to_owned(),
        icon_url: None,
        documentation_url: None,
        provider: None,
        capabilities: CapabilitiesConfig::default(),
        default_input_modes: vec!["text/plain".to_owned()],
        default_output_modes: vec!["text/plain".to_owned()],
        security_schemes: None,
        security: None,
        skills: Vec::new(),
        supports_authenticated_extended_card: false,
    }
}

// An agent that authors a (deprecated) `card.skills` entry referencing a skill
// id ("bar") that does NOT exist on disk, while declaring "foo" via
// `metadata.skills`. The bridge manifest must surface "foo" only.
fn phantom_agent() -> AgentConfig {
    let mut card = empty_card();
    card.skills = vec![AgentSkillConfig {
        id: SkillId::new("bar"),
        name: "Bar".to_owned(),
        description: "Phantom skill that lives only in YAML.".to_owned(),
        tags: Vec::new(),
        examples: None,
        input_modes: None,
        output_modes: None,
        security: None,
    }];

    AgentConfig {
        name: "phantom_agent".to_owned(),
        port: 9111,
        endpoint: "http://127.0.0.1:9111".to_owned(),
        tags: Vec::new(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        card,
        metadata: AgentMetadataConfig {
            skills: vec!["foo".to_owned()],
            ..AgentMetadataConfig::default()
        },
        oauth: OAuthConfig::default(),
    }
}

#[test]
fn manifest_skills_come_from_services_skills_dir_only() {
    let tmp = TempDir::new().expect("tempdir");
    write_skill_foo(tmp.path());

    let skills = load_skills(tmp.path()).expect("load_skills");
    let ids: Vec<String> = skills.iter().map(|s| s.id.as_str().to_owned()).collect();

    assert!(
        ids.iter().any(|id| id == "foo"),
        "expected 'foo' (from services/skills/foo) in manifest skills, got {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id == "bar"),
        "manifest must NOT include 'bar': it is only authored in agent card.skills, \
         not present in services/skills/. Got {ids:?}"
    );
}

#[test]
fn manifest_agent_entry_skills_mirror_metadata_skills() {
    let tmp = TempDir::new().expect("tempdir");
    write_skill_foo(tmp.path());

    let agent = phantom_agent();

    // Fixture sanity: the agent authors a phantom `card.skills` entry that
    // the bridge manifest must ignore.
    assert!(
        !agent.card.skills.is_empty(),
        "fixture should still tolerate (deprecated) card.skills in YAML"
    );
    assert_eq!(agent.card.skills[0].id.as_str(), "bar");
    assert_eq!(agent.metadata.skills, vec!["foo".to_owned()]);

    let mut services = ServicesConfig::default();
    services.agents.insert("phantom_agent".to_owned(), agent);

    let entries = load_agents(&services, "http://127.0.0.1");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(
        entry.skills,
        vec!["foo".to_owned()],
        "AgentEntry.skills must mirror metadata.skills, not card.skills"
    );
    assert!(
        !entry.skills.iter().any(|s| s == "bar"),
        "AgentEntry.skills must not leak card.skills ids"
    );
}

// --- Marketplace scoping (0.12.2) -------------------------------------------
//
// These tests cover the spec contract documented in
// `bridge_manifest::scope::scope_to_marketplace`: when an active marketplace
// is resolved, its `<entity>.include` list intersects the on-disk catalogue
// (empty list = global fallback, unknown ids are dropped silently, disk
// order is preserved). They exercise the helper directly against the
// `load_skills` output rather than driving the HTTP endpoint — the shared
// `OnceLock` bootstrap fixture cannot be reconfigured per test, and the
// helper is the unit that this PR introduces.

fn write_skill(services_root: &std::path::Path, id: &str) {
    let dir = services_root.join("skills").join(id);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    std::fs::write(
        dir.join("config.yaml"),
        format!("id: {id}\nname: {id}\ndescription: scoping fixture.\nenabled: true\ntags: []\n"),
    )
    .expect("write skill config");
    std::fs::write(dir.join("SKILL.md"), format!("{id} body.\n")).expect("write skill md");
}

#[test]
fn marketplace_include_filters_skills() {
    let tmp = TempDir::new().expect("tempdir");
    for id in ["foo", "bar", "baz"] {
        write_skill(tmp.path(), id);
    }

    let skills = load_skills(tmp.path()).expect("load_skills");
    // load_skills sorts directory entries by name; baseline is [bar, baz, foo].
    let baseline: Vec<String> = skills.iter().map(|s| s.id.as_str().to_owned()).collect();
    assert_eq!(baseline, vec!["bar", "baz", "foo"]);

    let include = vec!["foo".to_owned(), "baz".to_owned()];
    let scoped = scope_to_marketplace(skills, &include, |s| s.id.as_str());
    let ids: Vec<String> = scoped.iter().map(|s| s.id.as_str().to_owned()).collect();

    // Disk order preserved: baz comes before foo in the loader's sorted output.
    assert_eq!(ids, vec!["baz", "foo"]);
}

#[test]
fn empty_marketplace_include_serves_all() {
    let tmp = TempDir::new().expect("tempdir");
    for id in ["foo", "bar", "baz"] {
        write_skill(tmp.path(), id);
    }

    let skills = load_skills(tmp.path()).expect("load_skills");
    let scoped = scope_to_marketplace(skills, &[], |s| s.id.as_str());
    let ids: Vec<String> = scoped.iter().map(|s| s.id.as_str().to_owned()).collect();
    assert_eq!(ids, vec!["bar", "baz", "foo"]);
}

#[test]
fn nonexistent_id_in_include_is_dropped() {
    let tmp = TempDir::new().expect("tempdir");
    for id in ["foo", "bar", "baz"] {
        write_skill(tmp.path(), id);
    }

    let skills = load_skills(tmp.path()).expect("load_skills");
    let include = vec!["foo".to_owned(), "nonexistent".to_owned()];
    let scoped = scope_to_marketplace(skills, &include, |s| s.id.as_str());
    let ids: Vec<String> = scoped.iter().map(|s| s.id.as_str().to_owned()).collect();
    assert_eq!(ids, vec!["foo"]);
}
