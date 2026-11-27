use serde_json::json;
use systemprompt_engine::config::permission::{EDIT, SHELL};
use systemprompt_engine::config::{
    Action, AgentDef, AgentMode, ConfigError, Layout, Loaded, Origin, Ruleset, ToolFilter,
};

use crate::config::support::{load, load_with, paths, repo, write};

#[test]
fn tool_filters() {
    let listed = ToolFilter::from_value(&json!("Read, Grep")).unwrap();
    assert!(listed.permits("read"));
    assert!(!listed.permits("shell"));
    let map = ToolFilter::from_value(&json!({"shell": false, "mcp_*": false})).unwrap();
    assert!(map.permits("read"));
    assert!(!map.permits("shell"));
    assert!(!map.permits("mcp_github"));
    assert!(ToolFilter::from_value(&json!(3)).is_err());
}

#[test]
fn override_fields_from_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("harness.json"),
        r#"{"agent": {"rev": {
            "description": "d", "mode": "subagent", "model": "inherit", "temperature": 0.2,
            "maxSteps": 7, "maxOutput": 512, "hidden": true, "disallowedTools": ["shell"],
            "permission": {"edit": "deny"}, "color": "blue"
        }}}"#,
    );
    let loaded = load(&paths, &repo);
    let patch = &loaded.settings.agents["rev"];
    assert_eq!(patch.mode, Some(AgentMode::Subagent));
    assert_eq!(patch.model, None);
    assert_eq!(patch.steps, Some(7));
    assert_eq!(patch.max_output, Some(512));
    assert!(!patch.extra.contains_key("maxOutput"));
    assert_eq!(patch.hidden, Some(true));
    assert_eq!(
        patch.tools.as_ref().map(|t| t.deny.clone()),
        Some(vec!["shell".to_owned()])
    );
    assert_eq!(patch.permission.evaluate(EDIT, "x"), Action::Deny);
    assert_eq!(patch.extra["color"], "blue");
    let agent = &loaded.agents["rev"];
    assert_eq!(agent.description.as_deref(), Some("d"));
    assert_eq!(agent.temperature, Some(0.2));
    assert_eq!(agent.max_output, Some(512));
}

#[test]
fn an_output_limit_from_front_matter_and_its_absence() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let dir = paths.config.join("agents");
    write(
        &dir.join("brief.md"),
        "---
max_output: 16
---
Be brief.",
    );
    write(
        &dir.join("plain.md"),
        "---
steps: 2
---
No cap.",
    );
    let loaded = load(&paths, &repo);
    assert_eq!(loaded.agents["brief"].max_output, Some(16));
    assert_eq!(loaded.agents["plain"].max_output, None);
}

#[test]
fn an_output_limit_that_is_not_a_whole_number_fails_the_load() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("harness.json"),
        r#"{"agent": {"x": {"maxOutput": "lots"}}}"#,
    );
    let error = Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap_err();
    assert!(
        matches!(error, ConfigError::Field { ref key, .. } if key == "agent.x.maxOutput"),
        "{error:?}"
    );
}

#[test]
fn a_bad_override_fails_the_load() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("harness.json"),
        r#"{"agent": {"x": {"mode": "boss"}}}"#,
    );
    let error = Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap_err();
    assert!(
        matches!(error, ConfigError::Field { ref key, .. } if key == "agent.x.mode"),
        "{error:?}"
    );
}

#[test]
fn files_override_builtins_and_settings_override_files() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let dir = paths.config.join("agents");
    write(
        &dir.join("build.md"),
        "---\nmodel: m2\npermission:\n  shell: ask\n---\nCustom build prompt.\n",
    );
    write(
        &dir.join("reviewer.md"),
        "---\ndescription: Reviews\nmode: subagent\n---\nReview carefully.",
    );
    write(&dir.join("broken.md"), "---\nmode: [\n---\n");
    write(
        &dir.join(".hidden").join("ghost.md"),
        "---\ndescription: no\n---\n",
    );
    write(
        &paths.config.join("harness.json"),
        r#"{"agent": {"reviewer": {"steps": 3}, "plan": {"disable": true}}}"#,
    );

    let mut build = AgentDef::new("build", AgentMode::Primary);
    build.prompt = Some("Built-in.".to_owned());
    build.defaults = Ruleset::build_defaults(&paths);
    let plan = AgentDef::new("plan", AgentMode::Primary);
    let loaded = load_with(&paths, &repo, vec![build, plan]);

    assert_eq!(loaded.problems.len(), 1, "{:?}", loaded.problems);
    assert_eq!(
        loaded.agents.keys().collect::<Vec<_>>(),
        ["build", "reviewer"]
    );
    let build = &loaded.agents["build"];
    assert_eq!(build.prompt.as_deref(), Some("Custom build prompt."));
    assert_eq!(build.model.as_deref(), Some("m2"));
    assert_eq!(build.mode, AgentMode::Primary);
    assert_eq!(build.permission.evaluate(SHELL, "ls"), Action::Ask);
    assert!(matches!(build.origin, Origin::File(_)));
    let reviewer = &loaded.agents["reviewer"];
    assert_eq!(reviewer.mode, AgentMode::Subagent);
    assert_eq!(reviewer.steps, Some(3));
    assert_eq!(reviewer.origin, Origin::Settings);
    assert!(!reviewer.defaults.is_empty());
}
