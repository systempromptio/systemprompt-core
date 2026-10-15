//! Schema-2 skill bodies are read from the synced plugin SKILL.md and accepted
//! only when they hash to the skill's signed digest.

use sha2::{Digest, Sha256};
use systemprompt_bridge::gateway::manifest::SkillEntry;
use systemprompt_bridge::ids::{Sha256Digest, SkillId, SkillName};
use systemprompt_bridge::sync::apply::skill_bodies::body_from_plugin;
use systemprompt_models::bridge::manifest::skill_frontmatter::render_passthrough_frontmatter;

fn skill_signing(body: &str) -> SkillEntry {
    let frontmatter = crate::skill_passthrough::authored_frontmatter();
    let rendered = render_passthrough_frontmatter(Some(&frontmatter)).unwrap();
    let digest =
        systemprompt_bridge::hash::hex_encode(&Sha256::digest(format!("{rendered}{body}")));
    SkillEntry {
        publication: None,
        id: SkillId::try_new("code_review").unwrap(),
        name: SkillName::try_new("Code review").unwrap(),
        description: "Reviews code".into(),
        file_path: "skills/code_review/SKILL.md".into(),
        tags: vec![],
        sha256: Sha256Digest::try_new(digest).unwrap(),
        instructions: None,
        source: None,
        hosts: vec![],
        plugins: vec![],
        frontmatter: Some(frontmatter),
    }
}

fn plugin_with(skill_md: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("skills/code-review/SKILL.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, skill_md).unwrap();
    dir
}

#[test]
fn a_plugin_skill_md_body_matching_the_digest_is_taken() {
    let plugin = plugin_with(
        "---\nname: code-review\ndescription: \"Reviews code\"\nmodel: sonnet\n---\n\nStep one.\n\n---\nStep two.\n",
    );
    let body = body_from_plugin(
        &skill_signing("Step one.\n\n---\nStep two."),
        plugin.path(),
        "skills/code-review/SKILL.md",
    )
    .unwrap();
    assert_eq!(body, "Step one.\n\n---\nStep two.");
}

#[test]
fn a_body_that_does_not_match_the_digest_is_refused() {
    let plugin = plugin_with("---\nname: code-review\n---\n\nTampered.\n");
    let err = body_from_plugin(
        &skill_signing("Original."),
        plugin.path(),
        "skills/code-review/SKILL.md",
    )
    .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
}

#[test]
fn a_traversing_source_path_is_refused() {
    let plugin = plugin_with("---\n---\nx\n");
    assert!(body_from_plugin(&skill_signing("x"), plugin.path(), "../outside/SKILL.md").is_err());
}
