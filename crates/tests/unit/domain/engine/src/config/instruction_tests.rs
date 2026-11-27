use std::path::PathBuf;

use systemprompt_engine::config::{ConfigError, Layout, Loaded};

use crate::config::support::{paths, repo, write};

#[test]
fn order_fallback_cap_and_extras() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let root = repo(tmp.path());
    let sub = root.join("pkg");
    write(&paths.config.join("AGENTS.md"), "global rules");
    write(&paths.home.join(".claude/CLAUDE.md"), "home claude");
    write(&root.join("AGENTS.md"), "root agents");
    write(&root.join("CLAUDE.md"), "root claude, shadowed");
    write(&sub.join("CLAUDE.md"), "pkg claude ".repeat(20).as_str());
    write(&root.join("docs/style.md"), "style");
    write(&root.join("docs/deep/more.md"), "more");
    write(&root.join("docs/.hidden/no.md"), "no");
    write(
        &root.join("harness.json"),
        r#"{"instructions": ["docs/**/*.md", "missing.md", "docs/style.md"]}"#,
    );

    let layout = Layout {
        instruction_limit: 32,
        ..Layout::default()
    };
    let loaded = Loaded::load(&paths, &layout, &sub, Vec::new()).unwrap();
    let found = &loaded.instructions;

    let order: Vec<PathBuf> = found.iter().map(|i| i.path.clone()).collect();
    assert_eq!(
        order,
        [
            paths.config.join("AGENTS.md"),
            paths.home.join(".claude").join("CLAUDE.md"),
            root.join("AGENTS.md"),
            sub.join("CLAUDE.md"),
            root.join("docs").join("deep/more.md"),
            root.join("docs").join("style.md"),
        ]
    );
    let pkg = &found[3];
    assert!(pkg.truncated);
    assert_eq!(pkg.text.len(), 32);
    assert_eq!(pkg.bytes, 220);
    assert_eq!(loaded.problems.len(), 1, "{:?}", loaded.problems);
    assert!(matches!(
        &loaded.problems[0],
        ConfigError::InstructionMissing { .. }
    ));
}

#[test]
fn caps_on_a_char_boundary_and_reads_absolute_globs() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let root = repo(tmp.path());
    let notes = tmp.path().join("notes");
    write(&paths.config.join("AGENTS.md"), "ok");
    write(&root.join("AGENTS.md"), "h\u{e9}llo");
    write(&notes.join("one.md"), "one");
    let glob = format!("{}/*.md", notes.display()).replace('\\', "/");
    let settings = serde_json::json!({ "instructions": [glob] }).to_string();
    write(&root.join("harness.json"), &settings);

    let layout = Layout {
        instruction_limit: 2,
        ..Layout::default()
    };
    let loaded = Loaded::load(&paths, &layout, &root, Vec::new()).unwrap();
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    let found = &loaded.instructions;
    assert_eq!(found.len(), 3);
    assert_eq!((found[0].text.as_str(), found[0].truncated), ("ok", false));
    assert_eq!((found[1].text.as_str(), found[1].truncated), ("h", true));
    assert_eq!(found[1].bytes, 6);
    assert_eq!(found[2].path, notes.join("one.md"));
}
