//! What a skill body cites but the import will not carry: reported by default,
//! refused under strict.

use std::path::Path;

use systemprompt_marketplace::{ImportOptions, ImportWarning, import_anthropic_tree};
use tempfile::TempDir;

fn tree(files: &[(&str, &str)]) -> TempDir {
    let root = TempDir::new().expect("tempdir");
    let write = |rel: &str, body: &str| {
        let path = root.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write");
    };
    write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","owner":{"name":"Acme"},"plugins":[{"name":"alpha","category":"ops"}]}"#,
    );
    write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha"}"#,
    );
    for (rel, body) in files {
        write(rel, body);
    }
    root
}

fn skill(body: &str) -> String {
    format!("---\nname: a-skill\ndescription: What it does.\n---\n\n{body}\n")
}

fn warnings(source: &Path) -> Vec<ImportWarning> {
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source,
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .expect("lenient import succeeds")
    .warnings
}

fn strict_error(source: &Path) -> String {
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source,
        dest.path(),
        &ImportOptions {
            strict: true,
            ..ImportOptions::new(std::env::temp_dir())
        },
    )
    .expect_err("strict import refuses the tree")
    .to_string()
}

#[test]
fn a_cited_reference_that_exists_raises_nothing() {
    let source = tree(&[
        (
            "plugins/alpha/skills/a-skill/SKILL.md",
            &skill("See [the guide](references/guide.md)."),
        ),
        ("plugins/alpha/skills/a-skill/references/guide.md", "Guide."),
    ]);
    assert!(warnings(source.path()).is_empty());
}

#[test]
fn a_cited_reference_the_tree_lacks_is_reported_and_strict_refuses_it() {
    let source = tree(&[(
        "plugins/alpha/skills/a-skill/SKILL.md",
        &skill("Read `references/missing.md` first."),
    )]);
    assert_eq!(
        warnings(source.path()),
        vec![ImportWarning::MissingReference {
            skill: "a_skill".to_owned(),
            path: "references/missing.md".to_owned(),
        }]
    );
    assert!(strict_error(source.path()).contains("references/missing.md"));
}

#[test]
fn a_reference_excluded_as_a_dev_file_counts_as_missing() {
    let source = tree(&[
        (
            "plugins/alpha/skills/a-skill/SKILL.md",
            &skill("Data in (references/fixtures/sample.json)."),
        ),
        (
            "plugins/alpha/skills/a-skill/references/fixtures/sample.json",
            "{}",
        ),
    ]);
    assert!(
        warnings(source.path()).contains(&ImportWarning::MissingReference {
            skill: "a_skill".to_owned(),
            path: "references/fixtures/sample.json".to_owned(),
        })
    );
}

#[test]
fn a_path_into_a_sibling_skill_is_reported() {
    let source = tree(&[(
        "plugins/alpha/skills/a-skill/SKILL.md",
        &skill("Reuse [the other one](../b-skill/SKILL.md)."),
    )]);
    assert_eq!(
        warnings(source.path()),
        vec![ImportWarning::SiblingSkillPath {
            skill: "a_skill".to_owned(),
            path: "../b-skill/SKILL.md".to_owned(),
        }]
    );
}

#[test]
fn a_skill_directory_without_skill_md_is_reported_and_strict_refuses_it() {
    let source = tree(&[
        ("plugins/alpha/skills/a-skill/SKILL.md", &skill("Body.")),
        ("plugins/alpha/skills/half-done/notes.md", "Draft."),
    ]);
    assert_eq!(
        warnings(source.path()),
        vec![ImportWarning::MissingSkillManifest {
            plugin: "alpha".to_owned(),
            dir: "half-done".to_owned(),
        }]
    );
    assert!(strict_error(source.path()).contains("half-done"));
}
