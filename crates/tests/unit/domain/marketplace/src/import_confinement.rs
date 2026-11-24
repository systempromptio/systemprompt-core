//! Every metadata read beneath an import root is confined to it: a symbolic
//! link or a `..` path that resolves outside the tree refuses the import with
//! a confinement cause instead of reading what lies outside.

use std::os::unix::fs::symlink;
use std::path::Path;

use systemprompt_marketplace::{ImportOptions, import_anthropic_tree};
use tempfile::TempDir;

const MARKETPLACE: &str =
    r#"{"name":"acme","owner":{"name":"Acme"},"plugins":[{"name":"pkg","source":"./plugins/pkg"}]}"#;
const PLUGIN: &str = r#"{"name":"pkg","description":"A plugin."}"#;
const SKILL: &str = "---\nname: a-skill\ndescription: What it does.\n---\n\nBody.\n";
const SIDECAR: &str = "schema: 1\n";

type EscapeCase = (&'static str, fn(&Fixture));

struct Fixture {
    tree: TempDir,
    outside: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            tree: TempDir::new().expect("tree"),
            outside: TempDir::new().expect("outside"),
        };
        fixture.write(".claude-plugin/marketplace.json", MARKETPLACE);
        fixture.write("plugins/pkg/.claude-plugin/plugin.json", PLUGIN);
        fixture.write("plugins/pkg/skills/a-skill/SKILL.md", SKILL);
        for (rel, body) in [
            ("marketplace.json", MARKETPLACE),
            ("plugin.json", PLUGIN),
            ("systemprompt.yaml", SIDECAR),
            ("rule.md", "# Rule\n"),
            ("agent.md", "# Agent\n"),
            ("config.yaml", "settings: {}\n"),
            ("skill/SKILL.md", SKILL),
        ] {
            let path = fixture.outside.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, body).expect("write outside");
        }
        fixture
    }

    fn root(&self) -> &Path {
        self.tree.path()
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.root().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write");
    }

    fn link(&self, rel: &str, outside_rel: &str) {
        let path = self.root().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        if path.exists() {
            std::fs::remove_file(&path).expect("replace");
        }
        symlink(self.outside.path().join(outside_rel), path).expect("symlink");
    }
}

fn import(fixture: &Fixture) -> Result<(), String> {
    let dest = TempDir::new().expect("dest");
    import_anthropic_tree(
        fixture.root(),
        &dest.path().join("out"),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .map(|_| ())
    .map_err(|error| {
        let mut chain = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            chain.push_str(": ");
            chain.push_str(&cause.to_string());
            source = cause.source();
        }
        chain
    })
}

#[test]
fn the_unmodified_fixture_imports() {
    import(&Fixture::new()).expect("the fixture imports before any escape is planted");
}

#[test]
fn every_reader_refuses_a_path_that_escapes_the_tree() {
    let cases: [EscapeCase; 9] = [
        ("marketplace manifest", |f| {
            f.link(".claude-plugin/marketplace.json", "marketplace.json");
        }),
        ("marketplace sidecar", |f| {
            f.link(".claude-plugin/systemprompt.yaml", "systemprompt.yaml");
        }),
        ("plugin manifest", |f| {
            f.link("plugins/pkg/.claude-plugin/plugin.json", "plugin.json");
        }),
        ("plugin sidecar", |f| {
            f.link(
                "plugins/pkg/.claude-plugin/systemprompt.yaml",
                "systemprompt.yaml",
            );
        }),
        ("skills directory entry", |f| {
            f.link("plugins/pkg/skills/escaped", "skill");
        }),
        ("plugin rules entry", |f| {
            f.link("plugins/pkg/rules/escaped.md", "rule.md")
        }),
        ("plugin agents entry", |f| {
            f.link("plugins/pkg/agents/escaped.md", "agent.md")
        }),
        ("root rules entry", |f| {
            f.link("rules/escaped.md", "rule.md")
        }),
        ("base tree entry", |f| {
            f.link("systemprompt/agents/escaped.yaml", "config.yaml");
        }),
    ];
    for (reader, plant) in cases {
        let fixture = Fixture::new();
        plant(&fixture);
        let error = import(&fixture).expect_err(&format!(
            "{reader}: an escaping symlink must refuse the import"
        ));
        assert!(
            error.contains("resolves outside"),
            "{reader}: refused with a confinement cause, got: {error}"
        );
    }
}

#[test]
fn a_dot_dot_skill_path_that_leaves_the_tree_is_refused() {
    let fixture = Fixture::new();
    fixture.write(
        "plugins/pkg/.claude-plugin/plugin.json",
        r#"{"name":"pkg","description":"A plugin.","skills":"../../../escape"}"#,
    );
    let error = import(&fixture).expect_err("a .. skill path outside the tree must refuse");
    assert!(
        error.contains("must stay inside the plugin"),
        "refused as a confinement escape, got: {error}"
    );
}
