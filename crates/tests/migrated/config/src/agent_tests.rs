//! Agents from markdown files and settings: fields, rule order, disabling,
//! the default agent, and what built-in subagents may do.

use systemprompt_engine::config::permission::{EDIT, READ, SHELL, TASK};
use systemprompt_engine::config::{Action, AgentMode, Origin, Ruleset};
use systemprompt_engine::prompt::builtin;

use crate::support::Tree;

#[test]
fn a_markdown_agent_loads_its_fields_prompt_and_rules() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/reviewer.md",
        "---\ndescription: Reviews changes\nmode: subagent\nmodel: gateway/fast\ntemperature: 0.2\nsteps: 7\ntools: read, grep\npermission:\n  edit: deny\n---\nYou review code.\n",
    );
    let loaded = tree.load();
    let agent = loaded.agent("reviewer").unwrap();
    assert_eq!(agent.description.as_deref(), Some("Reviews changes"));
    assert_eq!(agent.mode, AgentMode::Subagent);
    assert_eq!(agent.model.as_deref(), Some("gateway/fast"));
    assert_eq!(agent.temperature, Some(0.2));
    assert_eq!(agent.steps, Some(7));
    assert_eq!(agent.prompt.as_deref(), Some("You review code."));
    assert!(agent.tools.permits("grep"));
    assert!(!agent.tools.permits("shell"));
    assert_eq!(agent.permission.evaluate(EDIT, "a.rs"), Action::Deny);
    assert!(matches!(&agent.origin, Origin::File(path) if path.ends_with("reviewer.md")));
}

#[test]
fn rules_in_an_agent_file_keep_their_source_order() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/ops.md",
        "---\npermission:\n  shell:\n    \"*\": deny\n    \"git *\": allow\n    \"git push *\": ask\n---\nOps.\n",
    );
    let loaded = tree.load();
    let rules = &loaded.agent("ops").unwrap().permission;
    let patterns: Vec<&str> = rules.iter().map(|rule| rule.pattern.as_str()).collect();
    assert_eq!(patterns, ["*", "git *", "git push *"]);
    assert_eq!(rules.evaluate(SHELL, "git push origin"), Action::Ask);
    assert_eq!(rules.evaluate(SHELL, "git log"), Action::Allow);
    assert_eq!(rules.evaluate(SHELL, "rm x"), Action::Deny);
}

#[test]
fn an_agent_file_with_flow_collections_and_an_unquoted_colon_still_loads() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/helper.md",
        "---\ndescription: Use when: the user asks for help\ntools: [read, grep]\nlabels: {team: core}\n---\nHelp.\n",
    );
    let loaded = tree.load();
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    let agent = loaded.agent("helper").unwrap();
    assert_eq!(
        agent.description.as_deref(),
        Some("Use when: the user asks for help")
    );
    assert!(agent.tools.permits("read"));
    assert!(!agent.tools.permits("edit"));
    assert_eq!(agent.extra["labels"]["team"], "core");
}

#[test]
fn a_temperature_with_and_without_other_sampling_keys_is_kept() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/a.md",
        "---\ntemperature: 0.7\nvariant: high\n---\nA.\n",
    );
    tree.project(".harness/agents/b.md", "---\ntemperature: 1\n---\nB.\n");
    let loaded = tree.load();
    assert_eq!(loaded.agent("a").unwrap().temperature, Some(0.7));
    assert_eq!(loaded.agent("a").unwrap().extra["variant"], "high");
    assert_eq!(loaded.agent("b").unwrap().temperature, Some(1.0));
}

#[test]
fn home_relative_rule_patterns_match_posix_and_windows_spellings_of_the_path() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/a.md",
        "---\npermission:\n  read:\n    \"~/private/*\": deny\n---\nA.\n",
    );
    let loaded = tree.load();
    let rules = &loaded.agent("a").unwrap().permission;
    let home = tree.paths.home.to_string_lossy().replace('\\', "/");
    assert_eq!(
        rules.evaluate(READ, &format!("{home}/private/key")),
        Action::Deny
    );
    let windows = format!("{home}/private/key").replace('/', "\\");
    assert_eq!(rules.evaluate(READ, &windows), Action::Deny);
}

#[test]
fn configured_agent_fields_override_and_an_unset_model_stays_unset() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"agent": {"build": {"description": "Mine", "steps": 3, "hidden": false, "max_output": 2048}, "plan": {"model": "inherit"}}}"#,
    );
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    assert_eq!(build.description.as_deref(), Some("Mine"));
    assert_eq!(build.steps, Some(3));
    assert_eq!(build.max_output, Some(2048));
    assert_eq!(build.origin, Origin::Settings);
    assert_eq!(loaded.agent("plan").unwrap().model, None);
}

#[test]
fn a_built_in_agent_disabled_in_settings_disappears() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"agent": {"plan": {"disable": true}}}"#);
    let loaded = tree.load();
    assert!(loaded.agent("plan").is_none());
    assert!(loaded.primary_agents().all(|agent| agent.name != "plan"));
}

#[test]
fn a_built_in_agent_disabled_by_an_agent_file_disappears() {
    let tree = Tree::new();
    tree.project(".harness/agents/general.md", "---\ndisable: true\n---\n");
    let loaded = tree.load();
    assert!(loaded.agent("general").is_none());
    assert_eq!(loaded.subagents().count(), 0);
}

#[test]
fn agent_files_load_from_the_global_and_compatibility_directories() {
    let tree = Tree::new();
    tree.global("agents/mine.md", "---\ndescription: global\n---\nG.\n");
    tree.put(
        &tree.paths.home.join(".claude/agents/compat.md"),
        "---\ndescription: compat\n---\nC.\n",
    );
    tree.project(
        ".claude/agents/local.md",
        "---\ndescription: local compat\n---\nL.\n",
    );
    let loaded = tree.load();
    assert_eq!(loaded.agent("mine").unwrap().mode, AgentMode::All);
    assert_eq!(loaded.agent("compat").unwrap().mode, AgentMode::Subagent);
    assert_eq!(loaded.agent("local").unwrap().mode, AgentMode::Subagent);
}

#[test]
fn the_default_agent_is_the_configured_one_when_usable_otherwise_the_first_primary() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"default_agent": "plan"}"#);
    assert_eq!(tree.load().default_agent().unwrap().name, "plan");

    let tree = Tree::new();
    tree.project("harness.json", r#"{"default_agent": "general"}"#);
    assert_eq!(tree.load().default_agent().unwrap().name, "build");

    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"default_agent": "missing", "agent": {"build": {"disable": true}}}"#,
    );
    assert_eq!(tree.load().default_agent().unwrap().name, "plan");
}

#[test]
fn a_custom_agent_counts_as_primary_only_when_its_mode_says_so() {
    let tree = Tree::new();
    tree.project(".claude/agents/helper.md", "---\ndescription: h\n---\nH.\n");
    tree.project(
        ".claude/agents/lead.md",
        "---\ndescription: l\nmode: primary\n---\nL.\n",
    );
    let loaded = tree.load();
    let primary: Vec<&str> = loaded
        .primary_agents()
        .map(|agent| agent.name.as_str())
        .collect();
    assert!(primary.contains(&"lead"));
    assert!(!primary.contains(&"helper"));
}

#[test]
#[ignore = "parity ledger: the agent listing keeps table order instead of putting the default agent first"]
fn the_agent_listing_puts_the_selected_default_agent_first() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"default_agent": "plan"}"#);
    let loaded = tree.load();
    assert_eq!(loaded.primary_agents().next().unwrap().name, "plan");
}

#[test]
fn built_in_subagents_cannot_start_further_subagents() {
    for name in ["general", "explore"] {
        let agent = builtin(name).unwrap();
        assert!(agent.denies(TASK), "{name}");
    }
    assert!(!builtin("build").unwrap().denies(TASK));
}

#[test]
fn an_agent_colour_is_kept_as_given() {
    let tree = Tree::new();
    tree.project(".harness/agents/a.md", "---\ncolor: accent\n---\nA.\n");
    tree.project(".harness/agents/b.md", "---\ncolor: \"#ff8800\"\n---\nB.\n");
    let loaded = tree.load();
    assert_eq!(loaded.agent("a").unwrap().extra["color"], "accent");
    assert_eq!(loaded.agent("b").unwrap().extra["color"], "#ff8800");
}

#[test]
fn an_agent_file_that_changes_makes_the_configuration_stale_and_a_reload_picks_it_up() {
    let tree = Tree::new();
    let file = tree.project(".harness/agents/a.md", "---\ndescription: one\n---\nA.\n");
    let loaded = tree.load();
    assert!(!loaded.is_stale());
    std::fs::write(&file, "---\ndescription: two\n---\nA.\n").unwrap();
    crate::support::touch_later(&file);
    assert!(loaded.is_stale());
    assert_eq!(
        tree.load().agent("a").unwrap().description.as_deref(),
        Some("two")
    );

    let added = tree.project(".harness/agents/b.md", "---\ndescription: b\n---\nB.\n");
    let reloaded = tree.load();
    assert!(reloaded.agent("b").is_some());
    std::fs::remove_file(&added).unwrap();
    assert!(reloaded.is_stale());
    assert!(tree.load().agent("b").is_none());
}

#[test]
fn a_change_outside_agent_sources_leaves_the_configuration_current() {
    let tree = Tree::new();
    tree.project(".harness/agents/a.md", "---\ndescription: one\n---\nA.\n");
    let loaded = tree.load();
    tree.project("src/lib.rs", "fn main() {}");
    tree.project(".harness/notes/readme.txt", "unrelated");
    assert!(!loaded.is_stale());
}

#[test]
fn the_session_rule_layer_sits_above_every_agent_rule() {
    let tree = Tree::new();
    tree.project(
        ".harness/agents/a.md",
        "---\npermission:\n  shell: deny\n---\nA.\n",
    );
    let loaded = tree.load();
    let agent = loaded.agent("a").unwrap();
    let session = Ruleset::from(vec![systemprompt_engine::config::Rule::new(
        SHELL,
        "ls",
        Action::Allow,
    )]);
    assert_eq!(loaded.evaluate(agent, &session, SHELL, "ls"), Action::Allow);
    assert_eq!(loaded.evaluate(agent, &session, SHELL, "rm"), Action::Deny);
}
