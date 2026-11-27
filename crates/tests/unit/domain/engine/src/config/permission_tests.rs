use std::path::{Path, PathBuf};

use serde_json::json;
use systemprompt_engine::config::permission::{
    EDIT, EXTERNAL_DIRECTORY, FETCH, GREP, READ, SHELL, wildcard,
};
use systemprompt_engine::config::{Action, InvalidRule, Paths, Rule, Ruleset};

fn paths() -> Paths {
    let dir = |name: &str| PathBuf::from("/root").join(name);
    Paths {
        home: dir("home"),
        data: dir("data"),
        config: dir("config"),
        cache: dir("cache"),
        state: dir("state"),
        tmp: dir("tmp"),
    }
}

#[test]
fn wildcards() {
    assert!(wildcard("*", ""));
    assert!(wildcard("*", "anything/at/all"));
    assert!(wildcard("git *", "git"));
    assert!(wildcard("git *", "git status --short"));
    assert!(!wildcard("git *", "gitk"));
    assert!(wildcard("src/*.rs", "src/a/b.rs"));
    assert!(wildcard("a?c", "abc"));
    assert!(!wildcard("a?c", "ac"));
    assert!(wildcard("*.env", "/p/.env"));
    assert!(!wildcard("*.env", "/p/.env.local"));
    assert!(wildcard("C:/work/*", "C:\\work\\x.txt"));
    assert!(wildcard("**", "x"));
    assert!(!wildcard("abc", "abcd"));
}

#[test]
fn last_match_wins_across_layers() {
    let global = Ruleset::from(vec![
        Rule::new(SHELL, "*", Action::Ask),
        Rule::new(SHELL, "git *", Action::Allow),
    ]);
    let project = Ruleset::from(vec![Rule::new(SHELL, "git push *", Action::Deny)]);
    let agent = Ruleset::from(vec![Rule::new(EDIT, "*", Action::Deny)]);
    let session = Ruleset::from(vec![Rule::new(EDIT, "/p/notes.md", Action::Allow)]);
    let rules = Ruleset::layered(&[&global, &project, &agent, &session]);
    assert_eq!(rules.evaluate(SHELL, "ls"), Action::Ask);
    assert_eq!(rules.evaluate(SHELL, "git status"), Action::Allow);
    assert_eq!(rules.evaluate(SHELL, "git push origin main"), Action::Deny);
    assert_eq!(rules.evaluate(EDIT, "/p/a.rs"), Action::Deny);
    assert_eq!(rules.evaluate(EDIT, "/p/notes.md"), Action::Allow);
    assert_eq!(rules.evaluate(FETCH, "https://x"), Action::Ask);
    assert_eq!(rules.evaluate_all(SHELL, ["git log", "ls"]), Action::Ask);
    assert_eq!(
        rules.evaluate_all(SHELL, ["git log", "git push x"]),
        Action::Deny
    );
    assert!(!rules.denies_all(EDIT));
    assert!(agent.denies_all(EDIT));
}

#[test]
fn rules_from_settings_shapes() {
    let home = Path::new("/home/u");
    let rules = Ruleset::from_value(
        &json!({"shell": {"*": "ask", "git *": "allow"}, "edit": "deny", "read": {"~/secret/*": "deny"}}),
        home,
    )
    .unwrap();
    assert_eq!(
        rules.rules(),
        &[
            Rule::new(SHELL, "*", Action::Ask),
            Rule::new(SHELL, "git *", Action::Allow),
            Rule::new(EDIT, "*", Action::Deny),
            Rule::new(READ, "/home/u/secret/*", Action::Deny),
        ]
    );
    assert_eq!(
        Ruleset::from_value(&json!("deny"), home)
            .unwrap()
            .evaluate("x", "y"),
        Action::Deny
    );
    let listed = Ruleset::from_value(
        &json!([{"permission": "fetch", "pattern": "https://*", "action": "allow"}]),
        home,
    )
    .unwrap();
    assert_eq!(listed.evaluate(FETCH, "https://example.com"), Action::Allow);
    assert!(matches!(
        Ruleset::from_value(&json!({"edit": "alow"}), home),
        Err(InvalidRule::Action { .. })
    ));
    assert!(matches!(
        Ruleset::from_value(&json!({"edit": 1}), home),
        Err(InvalidRule::Shape { .. })
    ));
    assert!(matches!(
        Ruleset::from_value(&json!(3), home),
        Err(InvalidRule::Root)
    ));
}

#[test]
fn default_sets() {
    let paths = paths();
    let build = Ruleset::build_defaults(&paths);
    assert_eq!(build.evaluate(EDIT, "/p/a.rs"), Action::Allow);
    assert_eq!(build.evaluate(READ, "/p/.env"), Action::Ask);
    assert_eq!(build.evaluate(READ, "/p/.env.example"), Action::Allow);
    assert_eq!(
        build.evaluate(EXTERNAL_DIRECTORY, "/elsewhere/x"),
        Action::Ask
    );
    assert_eq!(
        build.evaluate(EXTERNAL_DIRECTORY, "/root/tmp/x"),
        Action::Allow
    );
    let plan = Ruleset::plan_defaults(&paths);
    assert_eq!(plan.evaluate(EDIT, "/p/a.rs"), Action::Deny);
    assert_eq!(plan.evaluate(EDIT, "/root/state/plans/p.md"), Action::Allow);
    assert_eq!(plan.evaluate(SHELL, "cargo build"), Action::Ask);
    assert_eq!(plan.evaluate(SHELL, "git status"), Action::Allow);
    assert_eq!(plan.evaluate(GREP, "x"), Action::Allow);
}
