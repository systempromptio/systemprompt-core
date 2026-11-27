use harness_config::{expand_template, split_arguments};

use crate::support::{load, paths, repo, write};

#[test]
fn arguments_split_with_quotes() {
    assert_eq!(
        split_arguments(r#"a "b c"  'd e' f"#),
        ["a", "b c", "d e", "f"]
    );
    assert_eq!(split_arguments(r#"x "" y"#), ["x", "", "y"]);
    assert_eq!(split_arguments("   "), Vec::<String>::new());
}

#[test]
fn template_substitution() {
    assert_eq!(
        expand_template("Fix $1 in $2.", "bug42 src/lib.rs extra"),
        "Fix bug42 in src/lib.rs."
    );
    assert_eq!(expand_template("All: $ARGUMENTS!", "  a  b "), "All: a  b!");
    assert_eq!(expand_template("Missing [$3]", "one"), "Missing []");
    assert_eq!(expand_template("Costs $0 or $", "x"), "Costs $0 or $\n\nx");
    assert_eq!(
        expand_template("Review the diff.", "carefully"),
        "Review the diff.\n\ncarefully"
    );
    assert_eq!(expand_template("Review the diff.", ""), "Review the diff.");
    assert_eq!(expand_template("$1$1 \"$2\"", r#"a "b c""#), "aa \"b c\"");
}

#[test]
fn loads_nested_commands_with_front_matter() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let global = paths.config.join("commands");
    let project = repo.join(".harness").join("commands");
    write(
        &global.join("git").join("commit.md"),
        "---\ndescription: Commit\nsubtask: true\n---\nCommit $ARGUMENTS\n",
    );
    write(&global.join("review.md"), "Global review");
    write(
        &project.join("review.md"),
        "---\nagent: plan\nmodel: m\n---\nProject review of $1",
    );
    write(&project.join("bad.md"), "---\nsubtask: maybe\n---\n");

    let loaded = load(&paths, &repo);
    let commands = &loaded.commands;
    assert_eq!(
        commands.keys().collect::<Vec<_>>(),
        ["git/commit", "review"]
    );
    assert_eq!(commands["git/commit"].subtask, Some(true));
    assert_eq!(commands["git/commit"].render("now"), "Commit now");
    assert_eq!(commands["review"].agent.as_deref(), Some("plan"));
    assert_eq!(commands["review"].render("x y"), "Project review of x");
    assert_eq!(loaded.problems.len(), 1, "{:?}", loaded.problems);
}
