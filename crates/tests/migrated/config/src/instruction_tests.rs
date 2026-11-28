//! Instruction files: global and upward discovery, the compatibility file
//! name, configured files and globs, empty files and failing sources.

use crate::support::{Tree, touch_later};

fn names(tree: &Tree, loaded: &systemprompt_engine::config::Loaded) -> Vec<String> {
    let base = tree.repo.parent().unwrap();
    loaded
        .instructions
        .iter()
        .map(|file| {
            file.path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn global_then_upward_project_files_load_and_reload_on_change() {
    let tree = Tree::new();
    tree.global("AGENTS.md", "global");
    tree.project("AGENTS.md", "root");
    let nested = tree.project("pkg/AGENTS.md", "pkg");
    let cwd = tree.repo.join("pkg");
    let loaded = tree.load_at(&cwd);
    assert_eq!(
        names(&tree, &loaded),
        ["config/AGENTS.md", "repo/AGENTS.md", "repo/pkg/AGENTS.md"]
    );
    std::fs::write(&nested, "pkg changed").unwrap();
    touch_later(&nested);
    assert!(loaded.is_stale());
    assert_eq!(tree.load_at(&cwd).instructions[2].text, "pkg changed");
}

#[test]
fn the_compatibility_name_is_used_only_where_no_primary_file_exists() {
    let tree = Tree::new();
    tree.project("AGENTS.md", "primary");
    tree.project("CLAUDE.md", "fallback at root");
    tree.project("pkg/CLAUDE.md", "fallback in pkg");
    let loaded = tree.load_at(&tree.repo.join("pkg"));
    assert_eq!(
        names(&tree, &loaded),
        ["repo/AGENTS.md", "repo/pkg/CLAUDE.md"]
    );
}

#[test]
fn an_empty_instruction_file_is_kept_as_present() {
    let tree = Tree::new();
    tree.project("AGENTS.md", "");
    let loaded = tree.load();
    assert_eq!(loaded.instructions.len(), 1);
    assert_eq!(loaded.instructions[0].text, "");
}

#[test]
fn configured_files_globs_and_home_paths_load_alongside_the_discovered_file() {
    let tree = Tree::new();
    tree.project("AGENTS.md", "root");
    tree.project("docs/rules/a.md", "a");
    tree.project("docs/rules/b.md", "b");
    tree.project("STYLE.md", "style");
    tree.put(&tree.paths.home.join("notes.md"), "home");
    tree.project(
        "harness.json",
        r#"{"instructions": ["STYLE.md", "docs/rules/*.md", "~/notes.md"]}"#,
    );
    let loaded = tree.load();
    assert_eq!(
        names(&tree, &loaded),
        [
            "repo/AGENTS.md",
            "repo/STYLE.md",
            "repo/docs/rules/a.md",
            "repo/docs/rules/b.md",
            "home/notes.md"
        ]
    );
}

#[test]
fn a_new_instruction_file_is_discovered_on_the_next_load() {
    let tree = Tree::new();
    let loaded = tree.load();
    tree.project("AGENTS.md", "new");
    assert!(loaded.is_stale());
    assert_eq!(tree.load().instructions.len(), 1);
}

#[test]
fn discovery_stops_at_the_project_root() {
    let tree = Tree::new();
    tree.put(
        &tree.repo.parent().unwrap().join("AGENTS.md"),
        "above the root",
    );
    tree.project("AGENTS.md", "root");
    let loaded = tree.load();
    assert_eq!(names(&tree, &loaded), ["repo/AGENTS.md"]);
}

#[test]
fn a_missing_configured_file_is_reported_without_failing_the_load() {
    let tree = Tree::new();
    tree.project("AGENTS.md", "root");
    tree.project(
        "harness.json",
        r#"{"instructions": ["missing.md", "[bad-glob/*.md"]}"#,
    );
    let loaded = tree.load();
    assert_eq!(names(&tree, &loaded), ["repo/AGENTS.md"]);
    assert!(!loaded.problems.is_empty());
}

#[cfg(unix)]
#[test]
fn an_unreadable_instruction_file_fails_alone() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    tree.global("AGENTS.md", "global");
    let locked = tree.project("AGENTS.md", "secret");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&locked).is_ok() {
        // skip-ok: running as a user who can read any file, so nothing is unreadable
        return;
    }
    let loaded = tree.load();
    assert_eq!(names(&tree, &loaded), ["config/AGENTS.md"]);
    assert_eq!(loaded.problems.len(), 1);
}
