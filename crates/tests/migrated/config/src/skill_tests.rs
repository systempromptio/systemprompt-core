//! Skill directories: identifiers and flags from front matter, precedence
//! between sources, and noticing new or changed skill files.

use systemprompt_engine::config::Layout;

use crate::support::{Tree, touch_later};

fn skill(name: &str, description: &str, extra: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n{extra}---\nDo {name}.\n")
}

#[test]
fn a_skill_file_keeps_its_identifier_description_and_invocation_flags() {
    let tree = Tree::new();
    tree.project(
        ".harness/skills/deploy/SKILL.md",
        &skill("deploy", "Ships it", "disable-model-invocation: true\n"),
    );
    let loaded = tree.load();
    let deploy = &loaded.skills["deploy"];
    assert_eq!(deploy.description, "Ships it");
    assert_eq!(deploy.extra["disable-model-invocation"], true);
    assert_eq!(deploy.body.trim(), "Do deploy.");
    assert!(deploy.location.ends_with("deploy/SKILL.md"));
}

#[test]
fn a_skill_whose_name_differs_from_its_directory_is_refused() {
    let tree = Tree::new();
    tree.project(".harness/skills/a/SKILL.md", &skill("b", "x", ""));
    let loaded = tree.load();
    assert!(loaded.skills.is_empty());
    assert_eq!(loaded.problems.len(), 1);
}

#[test]
fn later_sources_win_over_earlier_ones() {
    let tree = Tree::new();
    tree.global("skills/lint/SKILL.md", &skill("lint", "global", ""));
    tree.put(
        &tree.paths.home.join(".agents/skills/lint/SKILL.md"),
        &skill("lint", "home compat", ""),
    );
    tree.project(
        ".claude/skills/lint/SKILL.md",
        &skill("lint", "project compat", ""),
    );
    assert_eq!(tree.load().skills["lint"].description, "project compat");
    tree.project(
        ".harness/skills/lint/SKILL.md",
        &skill("lint", "project", ""),
    );
    assert_eq!(tree.load().skills["lint"].description, "project");
}

#[test]
fn a_nested_checkout_copy_wins_over_the_parent_checkout() {
    let tree = Tree::new();
    tree.project(".harness/skills/fmt/SKILL.md", &skill("fmt", "parent", ""));
    let nested = tree.repo.join("worktrees/feature");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join(".git"),
        "gitdir: ../../.git/worktrees/feature\n",
    )
    .unwrap();
    tree.project(
        "worktrees/feature/.harness/skills/fmt/SKILL.md",
        &skill("fmt", "worktree", ""),
    );
    assert_eq!(tree.load_at(&nested).skills["fmt"].description, "worktree");
}

#[test]
fn a_skill_directory_is_rescanned_when_its_files_change() {
    let tree = Tree::new();
    let file = tree.project(".harness/skills/a/SKILL.md", &skill("a", "one", ""));
    let loaded = tree.load();
    std::fs::write(&file, skill("a", "two", "")).unwrap();
    touch_later(&file);
    assert!(loaded.is_stale());
    assert_eq!(tree.load().skills["a"].description, "two");
}

#[test]
fn skills_appearing_after_start_up_under_a_missing_root_are_picked_up() {
    let tree = Tree::new();
    let loaded = tree.load();
    assert!(loaded.skills.is_empty());
    tree.project(".harness/skills/late/SKILL.md", &skill("late", "late", ""));
    assert!(loaded.is_stale());
    assert!(tree.load().skills.contains_key("late"));
}

#[cfg(unix)]
#[test]
fn a_skill_reached_through_a_symlink_reloads_when_its_target_changes() {
    let tree = Tree::new();
    let target = tree.put(
        &tree.paths.data.join("shared/link/SKILL.md"),
        &skill("link", "one", ""),
    );
    let skills = tree.repo.join(".harness/skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::os::unix::fs::symlink(target.parent().unwrap(), skills.join("link")).unwrap();
    let loaded = tree.load();
    assert_eq!(loaded.skills["link"].description, "one");
    std::fs::write(&target, skill("link", "two", "")).unwrap();
    touch_later(&target);
    assert!(loaded.is_stale());
    assert_eq!(tree.load().skills["link"].description, "two");
}

#[test]
fn an_unrelated_settings_change_does_not_change_the_skill_set() {
    let tree = Tree::new();
    tree.project(".harness/skills/a/SKILL.md", &skill("a", "one", ""));
    let before = tree.load().skills;
    tree.project("harness.json", r#"{"model": "m"}"#);
    assert_eq!(tree.load().skills, before);
}

#[test]
fn compatibility_skill_directories_are_skipped_when_turned_off() {
    let tree = Tree::new();
    tree.project(".claude/skills/a/SKILL.md", &skill("a", "compat", ""));
    let layout = Layout {
        compat_dirs: false,
        ..Layout::default()
    };
    assert!(tree.load_with(&layout).skills.is_empty());
}
