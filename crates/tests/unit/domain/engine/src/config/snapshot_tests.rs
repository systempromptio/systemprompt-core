use std::path::Path;

use systemprompt_engine::config::permission::{EDIT, EXTERNAL_DIRECTORY, SHELL};
use systemprompt_engine::config::{
    Action, AgentDef, AgentMode, Layout, Loaded, Paths, Rule, Ruleset,
};

use crate::config::support::{paths, repo, write};

fn setup(root: &Path) -> (Paths, std::path::PathBuf) {
    (paths(root), repo(root))
}

fn builtins(paths: &Paths) -> Vec<AgentDef> {
    let mut build = AgentDef::new("build", AgentMode::Primary);
    build.defaults = Ruleset::build_defaults(paths);
    let mut plan = AgentDef::new("plan", AgentMode::Primary);
    plan.defaults = Ruleset::plan_defaults(paths);
    let mut general = AgentDef::new("general", AgentMode::Subagent);
    general.defaults = Ruleset::build_defaults(paths);
    vec![build, plan, general]
}

#[test]
fn loads_a_project_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, repo) = setup(tmp.path());
    let cwd = repo.join("src");
    write(
        &paths.config.join("harness.json"),
        r#"{"default_agent": "plan", "permission": {"shell": "ask"}}"#,
    );
    write(
        &repo.join(".harness/harness.json"),
        r#"{"permission": {"shell": {"cargo *": "allow"}}}"#,
    );
    write(
        &cwd.join("harness.json"),
        r#"{"agent": {"plan": {"hidden": true}}}"#,
    );
    write(
        &paths.home.join(".claude/agents/scout.md"),
        "---\ndescription: Finds things\n---\nScout.",
    );
    write(
        &repo.join(".harness/agents/docs.md"),
        "---\ndescription: Writes docs\nmode: primary\n---\nDocs.",
    );
    write(
        &paths.config.join("skills/tidy/SKILL.md"),
        "---\nname: tidy\ndescription: global\n---\nG",
    );
    write(
        &repo.join(".claude/skills/tidy/SKILL.md"),
        "---\nname: tidy\ndescription: project\n---\nP",
    );
    write(&repo.join(".harness/commands/ship.md"), "Ship $ARGUMENTS");
    write(&repo.join("AGENTS.md"), "Project rules.");
    write(
        &repo.join(".harness/agents/bad.md"),
        "---\nmode: sideways\n---\n",
    );

    let loaded = Loaded::load(&paths, &Layout::default(), &cwd, builtins(&paths)).unwrap();
    assert_eq!(loaded.worktree.root, repo);
    assert_eq!(loaded.settings_files.len(), 3);
    assert_eq!(loaded.agents["scout"].mode, AgentMode::Subagent);
    assert_eq!(loaded.agents["docs"].mode, AgentMode::Primary);
    assert!(loaded.agents["plan"].hidden);
    assert_eq!(
        loaded.default_agent().map(|agent| agent.name.as_str()),
        Some("build")
    );
    assert_eq!(loaded.skills["tidy"].description, "project");
    assert_eq!(loaded.commands["ship"].render("now"), "Ship now");
    assert_eq!(loaded.instructions.len(), 1);
    assert_eq!(loaded.problems.len(), 1, "{:?}", loaded.problems);
    evaluates_layered_rules(&loaded, &repo);

    assert!(!loaded.is_stale());
    write(
        &repo.join(".harness/skills/new/SKILL.md"),
        "---\nname: new\ndescription: n\n---\n",
    );
    assert!(loaded.is_stale());
    let reloaded = Loaded::load(&paths, &Layout::default(), &cwd, builtins(&paths)).unwrap();
    assert!(reloaded.skills.contains_key("new"));
    assert_ne!(reloaded.fingerprint(), loaded.fingerprint());
}

fn evaluates_layered_rules(loaded: &Loaded, repo: &Path) {
    let build = &loaded.agents["build"];
    let none = Ruleset::new();
    assert_eq!(
        loaded.evaluate(build, &none, SHELL, "cargo test"),
        Action::Allow
    );
    assert_eq!(loaded.evaluate(build, &none, SHELL, "rm x"), Action::Ask);
    let session = Ruleset::from(vec![Rule::new(SHELL, "rm *", Action::Allow)]);
    assert_eq!(
        loaded.evaluate(build, &session, SHELL, "rm x"),
        Action::Allow
    );
    let plan = &loaded.agents["plan"];
    assert_eq!(loaded.evaluate(plan, &none, EDIT, "/x/y.rs"), Action::Deny);
    let skill_file = repo.join(".claude/skills/tidy/SKILL.md");
    assert_eq!(
        loaded.evaluate(
            build,
            &none,
            EXTERNAL_DIRECTORY,
            &skill_file.to_string_lossy()
        ),
        Action::Allow
    );
}

#[test]
fn a_broken_settings_file_fails_the_load() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, repo) = setup(tmp.path());
    write(
        &repo.join("harness.json"),
        "{\"permission\": {\"edit\": \"deny\"",
    );
    let error = Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap_err();
    assert_eq!(error.path(), Some(repo.join("harness.json").as_path()));
}

#[test]
fn compat_dirs_can_be_turned_off() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, repo) = setup(tmp.path());
    write(
        &repo.join(".claude/skills/x/SKILL.md"),
        "---\nname: x\ndescription: d\n---\n",
    );
    write(&repo.join("CLAUDE.md"), "claude");
    let layout = Layout {
        compat_dirs: false,
        ..Layout::default()
    };
    let loaded = Loaded::load(&paths, &layout, &repo, Vec::new()).unwrap();
    assert!(loaded.skills.is_empty());
    assert_eq!(
        loaded.instructions,
        Vec::<systemprompt_engine::config::Instruction>::new()
    );
}
