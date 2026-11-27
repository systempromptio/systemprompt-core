use harness_config::{AgentDef, ConfigError};

use crate::support::{load, load_with, paths, repo, write};

#[test]
fn block_and_body() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("agents").join("reviewer.md"),
        "---\ndescription: Reviews code\nsteps: 4\ntemperature: 0.5\ntools:\n  shell: false\n---\nYou review.\n",
    );
    let loaded = load_with(&paths, &repo, Vec::<AgentDef>::new());
    let agent = &loaded.agents["reviewer"];
    assert_eq!(agent.description.as_deref(), Some("Reviews code"));
    assert_eq!(agent.steps, Some(4));
    assert_eq!(agent.temperature, Some(0.5));
    assert!(!agent.tools.permits("shell"));
    assert!(agent.tools.permits("read"));
    assert_eq!(agent.prompt.as_deref(), Some("You review."));
}

#[test]
fn without_front_matter() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let dir = paths.config.join("commands");
    write(&dir.join("plain.md"), "plain text");
    write(&dir.join("empty.md"), "");
    write(&dir.join("dashes.md"), "----\nx");
    write(&dir.join("bom.md"), "\u{feff}---\r\n# note\r\n---\r\nbody");
    let loaded = load(&paths, &repo);
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    assert_eq!(loaded.commands["plain"].template, "plain text");
    assert_eq!(loaded.commands["empty"].template, "");
    assert_eq!(loaded.commands["dashes"].template, "----\nx");
    assert_eq!(loaded.commands["bom"].template, "body");
    assert_eq!(loaded.commands["bom"].description, None);
}

#[test]
fn unclosed_and_invalid_blocks_name_a_line() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let dir = paths.config.join("commands");
    write(&dir.join("open.md"), "---\nname: x\nbody");
    write(&dir.join("invalid.md"), "---\nname: x\nlist: [1, 2\n---\n");
    write(&dir.join("list.md"), "---\n- a\n- b\n---\n");
    let loaded = load(&paths, &repo);
    let problems = &loaded.problems;
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(
        problems
            .iter()
            .any(|p| matches!(p, ConfigError::UnterminatedFrontMatter { line: 1, .. }))
    );
    let invalid = problems
        .iter()
        .find(|p| matches!(p, ConfigError::FrontMatter { .. }))
        .unwrap();
    assert!(invalid.line().is_some_and(|line| line >= 2));
    assert!(
        problems
            .iter()
            .any(|p| matches!(p, ConfigError::FrontMatterShape { .. }))
    );
    assert!(loaded.commands.is_empty());
}

#[test]
fn colon_in_plain_value_is_repaired() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("skills").join("helper").join("SKILL.md"),
        "---\nname: helper\ndescription: Use when: tests fail\n---\nbody",
    );
    let loaded = load(&paths, &repo);
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    assert_eq!(loaded.skills["helper"].description, "Use when: tests fail");
    assert_eq!(loaded.skills["helper"].name, "helper");
}
