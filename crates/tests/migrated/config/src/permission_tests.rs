//! Permission rules: the wildcard grammar, last-match precedence across
//! layers, deny checks, and how settings spell rules.

use serde_json::json;
use systemprompt_engine::config::permission::{EDIT, EXTERNAL_DIRECTORY, READ, SHELL, wildcard};
use systemprompt_engine::config::{Action, AgentDef, AgentMode, Rule, Ruleset};

use crate::support::Tree;

fn rules(value: serde_json::Value) -> Ruleset {
    Ruleset::from_value(&value, std::path::Path::new("/home/me")).unwrap()
}

#[test]
fn a_star_matches_any_run_of_characters_including_none_and_line_breaks() {
    assert!(wildcard("*", "tabs.list"));
    assert!(wildcard("*", ""));
    assert!(wildcard("tabs.*", "tabs.list"));
    assert!(wildcard("tabs.*", "tabs."));
    assert!(!wildcard("tabs.*", "files.upload"));
    assert!(wildcard("*.env", "a/b/c.env"));
    assert!(wildcard("first*", "first\nsecond"));
}

#[test]
fn a_question_mark_matches_exactly_one_character() {
    assert!(wildcard("tabs.ope?", "tabs.open"));
    assert!(!wildcard("tabs.ope?", "tabs.ope"));
    assert!(!wildcard("tabs.ope?", "tabs.opens"));
}

#[test]
fn a_trailing_space_star_also_matches_the_bare_command() {
    assert!(wildcard("git push *", "git push"));
    assert!(wildcard("git push *", "git push origin dev"));
    assert!(!wildcard("git push *", "git pushx"));
    assert!(wildcard("git push*", "git push"));
    assert!(wildcard("git push*", "git pushx"));
}

#[test]
fn backslashes_count_as_slashes_in_subject_and_pattern() {
    assert!(wildcard(
        "C:/Users/x/project/*",
        "C:\\Users\\x\\project\\a.ts"
    ));
    assert!(wildcard("C:\\Users\\x\\*", "C:/Users/x/project/a.ts"));
    assert!(!wildcard("C:/Users/x/*", "C:\\Users\\y\\a.ts"));
}

#[test]
fn every_other_character_is_literal() {
    assert!(!wildcard("b.owser", "browser"));
    assert!(wildcard("b.owser", "b.owser"));
    assert!(wildcard("a+b(c)[d]{e}|f^g$h", "a+b(c)[d]{e}|f^g$h"));
    assert!(!wildcard("browser", "browsers"));
    assert!(!wildcard("browser", "a browser"));
}

#[test]
fn letter_case_counts_except_where_the_file_system_ignores_it() {
    let folded = cfg!(windows);
    assert_eq!(wildcard(".harness/*", ".HARNESS/x"), folded);
    assert_eq!(wildcard("tabs.list", "Tabs.List"), folded);
}

#[test]
fn explicit_rules_allow_and_deny_without_asking_and_unmatched_subjects_ask() {
    let set = rules(json!({"shell": {"*": "deny", "git *": "allow"}, "read": "allow"}));
    assert_eq!(set.evaluate(SHELL, "git status"), Action::Allow);
    assert_eq!(set.evaluate(SHELL, "rm -rf /"), Action::Deny);
    assert_eq!(set.evaluate(READ, "src/main.rs"), Action::Allow);
    assert_eq!(set.evaluate("fetch", "https://example.test"), Action::Ask);
}

#[test]
#[ignore = "parity ledger: a narrow deny after a blanket deny hides the blanket deny from the deny-all check"]
fn a_blanket_deny_followed_only_by_narrow_denies_still_denies_everything() {
    let set = rules(json!([
        {"permission": "edit", "pattern": "*", "action": "deny"},
        {"permission": "edit", "pattern": "*.lock", "action": "deny"},
    ]));
    assert!(set.denies_all(EDIT));
}

#[test]
fn a_blanket_deny_lifted_by_a_later_allow_no_longer_denies_everything() {
    let lifted = rules(json!([
        {"permission": "edit", "pattern": "*", "action": "deny"},
        {"permission": "edit", "pattern": "docs/*", "action": "allow"},
    ]));
    assert!(!lifted.denies_all(EDIT));
    assert_eq!(lifted.evaluate(EDIT, "docs/a.md"), Action::Allow);
}

#[test]
fn rules_written_with_backslashes_match_slash_normalised_subjects() {
    let set = rules(json!({"edit": {"C:\\work\\*": "allow"}}));
    assert_eq!(set.evaluate(EDIT, "C:/work/a.rs"), Action::Allow);
}

#[test]
fn file_rules_ignore_letter_case_only_on_case_insensitive_systems() {
    let set = rules(json!({"edit": {"*": "allow", "Secrets/*": "deny"}}));
    let expected = if cfg!(windows) {
        Action::Deny
    } else {
        Action::Allow
    };
    assert_eq!(set.evaluate(EDIT, "secrets/key.pem"), expected);
}

#[test]
fn a_later_allow_lifts_an_earlier_broad_deny_and_a_bare_star_denies_everything() {
    let set = rules(json!([
        {"permission": "*", "pattern": "*", "action": "deny"},
        {"permission": "read", "pattern": "*", "action": "allow"},
    ]));
    assert_eq!(set.evaluate(READ, "a.txt"), Action::Allow);
    assert_eq!(set.evaluate(EDIT, "a.txt"), Action::Deny);
    assert_eq!(rules(json!("deny")).evaluate(SHELL, "ls"), Action::Deny);
}

#[test]
fn global_asks_and_denies_survive_a_project_that_says_nothing_about_them() {
    let tree = Tree::new();
    tree.global(
        "harness.json",
        r#"{"permission": {"shell": {"rm *": "deny", "curl *": "ask"}}}"#,
    );
    tree.project(
        "harness.json",
        r#"{"permission": {"shell": {"git *": "allow"}}}"#,
    );
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    let session = Ruleset::new();
    assert_eq!(
        loaded.evaluate(build, &session, SHELL, "rm -rf x"),
        Action::Deny
    );
    assert_eq!(
        loaded.evaluate(build, &session, SHELL, "curl x"),
        Action::Ask
    );
    assert_eq!(
        loaded.evaluate(build, &session, SHELL, "git log"),
        Action::Allow
    );
}

#[test]
fn session_rules_apply_after_agent_rules() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"agent": {"build": {"permission": {"shell": "ask"}}}}"#,
    );
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    let session = Ruleset::from(vec![Rule::new(SHELL, "cargo *", Action::Allow)]);
    assert_eq!(
        loaded.evaluate(build, &session, SHELL, "cargo test"),
        Action::Allow
    );
    assert_eq!(loaded.evaluate(build, &session, SHELL, "make"), Action::Ask);
}

#[test]
#[ignore = "parity ledger: a session approval layered last overrides a configured deny"]
fn a_saved_approval_never_overrides_a_configured_deny() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"permission": {"shell": {"rm *": "deny"}}}"#,
    );
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    let approvals = Ruleset::from(vec![Rule::new(SHELL, "*", Action::Allow)]);
    assert_eq!(
        loaded.evaluate(build, &approvals, SHELL, "rm -rf x"),
        Action::Deny
    );
}

#[test]
fn the_engine_temporary_directory_is_reachable_without_opening_other_external_directories() {
    let tree = Tree::new();
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    let session = Ruleset::new();
    let spilled = format!(
        "{}/out.txt",
        tree.paths.tmp.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        loaded.evaluate(build, &session, EXTERNAL_DIRECTORY, &spilled),
        Action::Allow
    );
    assert_eq!(
        loaded.evaluate(build, &session, EXTERNAL_DIRECTORY, "/elsewhere/x"),
        Action::Ask
    );
}

#[test]
fn environment_files_ask_by_default_except_the_example_file() {
    let tree = Tree::new();
    let loaded = tree.load();
    let build = loaded.agent("build").unwrap();
    let session = Ruleset::new();
    assert_eq!(
        loaded.evaluate(build, &session, READ, "/repo/.env"),
        Action::Ask
    );
    assert_eq!(
        loaded.evaluate(build, &session, READ, "/repo/.env.local"),
        Action::Ask
    );
    assert_eq!(
        loaded.evaluate(build, &session, READ, "/repo/.env.example"),
        Action::Allow
    );
    assert_eq!(
        loaded.evaluate(build, &session, READ, "/repo/src/a.rs"),
        Action::Allow
    );
}

#[test]
fn home_relative_patterns_expand_to_the_home_directory() {
    let set = Ruleset::from_value(
        &json!({"read": {"~/notes/*": "deny"}}),
        std::path::Path::new("/home/me"),
    )
    .unwrap();
    assert_eq!(set.evaluate(READ, "/home/me/notes/a.md"), Action::Deny);
    let windows = Ruleset::from_value(
        &json!({"read": {"~\\notes\\*": "deny"}}),
        std::path::Path::new("/home/me"),
    )
    .unwrap();
    assert_eq!(windows.evaluate(READ, "/home/me/notes/a.md"), Action::Deny);
}

#[test]
fn rules_keep_their_written_order_through_a_round_trip() {
    let set = rules(json!([
        {"permission": "shell", "pattern": "*", "action": "ask"},
        {"permission": "shell", "pattern": "git *", "action": "allow"},
        {"permission": "edit", "pattern": "*.lock", "action": "deny"},
    ]));
    let encoded = serde_json::to_value(&set).unwrap();
    let decoded: Ruleset = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, set);
    assert_eq!(decoded.rules()[1].pattern, "git *");
}

#[test]
fn an_unknown_action_spelling_is_refused_at_load() {
    let home = std::path::Path::new("/home/me");
    assert!(Ruleset::from_value(&json!({"shell": "permit"}), home).is_err());
    assert!(Ruleset::from_value(&json!({"shell": {"*": "yes"}}), home).is_err());
    assert!(
        Ruleset::from_value(
            &json!([{"permission": "shell", "pattern": "*", "action": "never"}]),
            home
        )
        .is_err()
    );
    let tree = Tree::new();
    tree.project("harness.json", r#"{"permission": {"shell": "sometimes"}}"#);
    let refused = systemprompt_engine::config::Loaded::load(
        &tree.paths,
        &systemprompt_engine::config::Layout::default(),
        &tree.repo,
        Vec::new(),
    );
    assert!(refused.is_err());
}

#[test]
fn global_rules_apply_before_agent_specific_ones() {
    let tree = Tree::new();
    tree.global("harness.json", r#"{"permission": {"edit": "deny"}}"#);
    tree.project(
        "harness.json",
        r#"{"agent": {"build": {"permission": {"edit": "allow"}}}}"#,
    );
    let loaded = tree.load();
    let session = Ruleset::new();
    let build = loaded.agent("build").unwrap();
    let plan = loaded.agent("plan").unwrap();
    assert_eq!(
        loaded.evaluate(build, &session, EDIT, "a.rs"),
        Action::Allow
    );
    assert_eq!(loaded.evaluate(plan, &session, EDIT, "a.rs"), Action::Deny);
}

#[test]
fn in_planning_mode_only_the_plan_directory_may_be_written() {
    let tree = Tree::new();
    let plan = Ruleset::plan_defaults(&tree.paths);
    let plans = tree.paths.state.join("plans").join("p.md");
    let plans = plans.to_string_lossy().replace('\\', "/");
    assert_eq!(plan.evaluate(EDIT, &plans), Action::Allow);
    assert_eq!(plan.evaluate(EDIT, "/repo/src/main.rs"), Action::Deny);
    assert_eq!(plan.evaluate(SHELL, "git status"), Action::Allow);
    assert_eq!(plan.evaluate(SHELL, "cargo build"), Action::Ask);
}

#[test]
fn built_in_agents_gain_no_shell_access_from_an_external_directory_allowance() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"permission": {"external_directory": "allow"}}"#,
    );
    let loaded = tree.load();
    let mut plan = AgentDef::new("planner", AgentMode::Primary);
    plan.defaults = Ruleset::plan_defaults(&tree.paths);
    let session = Ruleset::new();
    assert_eq!(
        loaded.evaluate(&plan, &session, EXTERNAL_DIRECTORY, "/x/y"),
        Action::Allow
    );
    assert_eq!(
        loaded.evaluate(&plan, &session, SHELL, "rm -rf /"),
        Action::Ask
    );
    assert_eq!(
        loaded.evaluate(&plan, &session, EDIT, "src/a.rs"),
        Action::Deny
    );
}

#[test]
fn building_the_planning_rules_creates_no_plan_directory() {
    let tree = Tree::new();
    let plan = Ruleset::plan_defaults(&tree.paths);
    assert_eq!(plan.evaluate(EDIT, "/repo/src/main.rs"), Action::Deny);
    assert!(tree.load().settings.model.is_none());
    assert!(!tree.paths.state.join("plans").exists());
}
