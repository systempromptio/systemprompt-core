//! Slash command files: reloading after edits, and template expansion with
//! literal arguments.

use systemprompt_engine::config::expand_template;

use crate::support::{Tree, touch_later};

#[test]
fn adding_changing_and_deleting_a_command_file_shows_on_the_next_load() {
    let tree = Tree::new();
    let first = tree.project(
        ".harness/commands/one.md",
        "---\ndescription: first\n---\nDo one.\n",
    );
    let loaded = tree.load();
    assert_eq!(loaded.commands.keys().collect::<Vec<_>>(), ["one"]);

    let second = tree.project(".harness/commands/git/two.md", "Do two.\n");
    assert!(loaded.is_stale());
    let loaded = tree.load();
    assert_eq!(
        loaded.commands.keys().collect::<Vec<_>>(),
        ["git/two", "one"]
    );

    std::fs::write(&first, "---\ndescription: changed\n---\nDo one again.\n").unwrap();
    touch_later(&first);
    assert!(loaded.is_stale());
    let loaded = tree.load();
    assert_eq!(
        loaded.commands["one"].description.as_deref(),
        Some("changed")
    );

    std::fs::remove_file(&second).unwrap();
    assert!(loaded.is_stale());
    assert_eq!(tree.load().commands.keys().collect::<Vec<_>>(), ["one"]);
}

#[test]
fn a_burst_of_edits_is_seen_as_one_change_with_the_final_content() {
    let tree = Tree::new();
    let file = tree.project(".harness/commands/c.md", "v0\n");
    let loaded = tree.load();
    for version in 1..=5 {
        std::fs::write(&file, format!("v{version}\n")).unwrap();
    }
    touch_later(&file);
    assert!(loaded.is_stale());
    let reloaded = tree.load();
    assert_eq!(reloaded.commands["c"].template, "v5");
    assert!(!reloaded.is_stale());
}

#[test]
fn a_change_outside_command_directories_leaves_commands_current() {
    let tree = Tree::new();
    tree.project(".harness/commands/c.md", "c\n");
    let loaded = tree.load();
    tree.project("docs/commands/other.md", "not a command\n");
    tree.project("README.md", "readme\n");
    assert!(!loaded.is_stale());
}

#[test]
fn a_command_written_right_after_the_first_load_is_picked_up() {
    let tree = Tree::new();
    let loaded = tree.load();
    tree.project(".harness/commands/fresh.md", "fresh\n");
    assert!(loaded.is_stale());
    assert!(tree.load().commands.contains_key("fresh"));
}

#[test]
fn typed_arguments_are_inserted_literally() {
    let expanded = expand_template("Review $ARGUMENTS now", "$1 and ${HOME} `rm`");
    assert_eq!(expanded, "Review $1 and ${HOME} `rm` now");
    let positional = expand_template("first=$1 second=$2", "\"a b\" c");
    assert_eq!(positional, "first=a b second=c");
}

#[test]
fn a_command_names_its_agent_model_and_subtask_choice() {
    let tree = Tree::new();
    tree.project(
        ".harness/commands/review.md",
        "---\nagent: plan\nmodel: gateway/fast\nsubtask: true\n---\nReview $ARGUMENTS\n",
    );
    tree.project(
        ".harness/commands/inline.md",
        "---\nsubtask: false\n---\nInline\n",
    );
    let loaded = tree.load();
    let review = &loaded.commands["review"];
    assert_eq!(review.agent.as_deref(), Some("plan"));
    assert_eq!(review.model.as_deref(), Some("gateway/fast"));
    assert_eq!(review.subtask, Some(true));
    assert_eq!(review.render("the diff"), "Review the diff");
    assert_eq!(loaded.commands["inline"].subtask, Some(false));
}
