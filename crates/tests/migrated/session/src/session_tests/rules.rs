//! Skills, rules and files outside the project: what an organisation's
//! denial and a repeated denial do to the skills listing, rules reloaded from
//! a changed settings file, and what a read or a patch asks the user.

use serde_json::json;
use systemprompt_engine::run::PermissionAsk;
use systemprompt_engine::store::ToolState;

use super::helpers::{add_skill, asking_before_reads, outside, run, run_approving};
use crate::support::{Scripted, Setup, read_turn, text_turn, tool_states, tool_turn};

fn with_settings(settings: &str) -> Setup {
    Setup {
        settings: Some(settings.to_owned()),
        ..Setup::default()
    }
}

async fn skills_listing(setup: Setup) -> String {
    let scripted = Scripted::with(vec![text_turn("hi")], setup).await;
    add_skill(&scripted);
    let id = scripted.session().await;
    run(&scripted, &id, "hello").await;
    let system = scripted.model_bodies()[0]["system"].to_string();
    scripted.finish().await;
    system
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an organisation's denial of a skill is given when the skill is called; the skill stays listed for the model"]
async fn a_skill_the_organisation_denies_is_no_longer_listed() {
    let system = skills_listing(Setup {
        deny: Some("skills are switched off by policy".to_owned()),
        ..Setup::default()
    })
    .await;
    assert!(!system.contains("Ships a release"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: skills are listed whenever the skill tool is offered; a skill whose own rule denies it again stays listed"]
async fn a_skill_allowed_and_then_denied_again_is_not_listed() {
    let rules = r#"{"permission": [
        {"permission": "skill", "pattern": "*", "action": "deny"},
        {"permission": "skill", "pattern": "deploy", "action": "allow"},
        {"permission": "skill", "pattern": "deploy", "action": "deny"}
    ]}"#;
    let system = skills_listing(with_settings(rules)).await;
    assert!(!system.contains("Ships a release"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rule_changed_in_the_settings_file_applies_to_the_next_prompt() {
    let scripted = Scripted::with(
        vec![
            read_turn("r1"),
            text_turn("Denied."),
            read_turn("r2"),
            text_turn("Read."),
        ],
        with_settings(r#"{"permission": {"read": "deny"}}"#),
    )
    .await;
    let id = scripted.session().await;
    run(&scripted, &id, "Read the notes").await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Error { .. }]),
        "{states:?}"
    );
    std::fs::write(
        scripted.project.join("harness.json"),
        r#"{"permission": {"read": "allow"}}"#,
    )
    .unwrap();
    run(&scripted, &id, "Read them again").await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(
            states.as_slice(),
            [ToolState::Error { .. }, ToolState::Completed { output, .. }]
                if output.contains("hello from the notes file")
        ),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_read_is_asked_about_under_the_resolved_file_and_the_call_that_made_it() {
    let scripted = Scripted::with(
        vec![
            tool_turn("call-7", "read", json!({"path": "sub/../notes.txt"})),
            text_turn("ok"),
        ],
        asking_before_reads(),
    )
    .await;
    let id = scripted.session().await;
    let settled = run_approving(&scripted, &id, "Read the notes").await;
    let [ask] = settled.asks.as_slice() else {
        panic!("{:?}", settled.asks);
    };
    assert_eq!(ask.permission, "read");
    assert_eq!(ask.patterns, ["notes.txt"]);
    assert_eq!(ask.session, id);
    assert_eq!(ask.call_id.as_str(), "call-7");
    assert_eq!(ask.tool, "read");
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(
            states.as_slice(),
            [ToolState::Completed { output, .. }] if output.contains("hello from the notes file")
        ),
        "{states:?}"
    );
    scripted.finish().await;
}

fn asked(asks: &[PermissionAsk]) -> Vec<&str> {
    asks.iter().map(|ask| ask.permission.as_str()).collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a name that differs only in unusual spaces is not recovered, so no second read is asked under the directory approval already given"]
async fn a_recovered_sibling_is_read_under_the_directory_approval_of_the_name_asked_for() {
    let scripted = Scripted::with(
        vec![
            tool_turn(
                "r1",
                "read",
                json!({"path": "../elsewhere/report final.txt"}),
            ),
            tool_turn("r2", "read", json!({"path": "../elsewhere/notes.txt"})),
            text_turn("done"),
        ],
        asking_before_reads(),
    )
    .await;
    let elsewhere = outside(&scripted, "elsewhere");
    std::fs::write(elsewhere.join("report\u{202f}final.txt"), "found\n").unwrap();
    std::fs::write(elsewhere.join("notes.txt"), "other notes\n").unwrap();
    let id = scripted.session().await;
    let settled = run_approving(&scripted, &id, "Read the report, then the notes").await;
    assert_eq!(
        asked(&settled.asks),
        [
            "external_directory",
            "read",
            "read",
            "external_directory",
            "read"
        ]
    );
    assert!(
        settled.asks[2].patterns[0].ends_with("report\u{202f}final.txt"),
        "{:?}",
        settled.asks[2].patterns
    );
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(
            states.as_slice(),
            [ToolState::Completed { output, .. }, ToolState::Completed { .. }]
                if output.contains("found")
        ),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: each file outside the project is asked about on its own, and an always answer remembers that file's directory, not its repository"]
async fn files_outside_the_project_are_asked_about_together_by_directory() {
    let patch = "%% begin patch\n\
        %% add ../elsewhere/first-repo/nested/a.txt\n+one\n\
        %% add inside.txt\n+two\n\
        %% add ../elsewhere/first-repo/nested/b.txt\n+three\n\
        %% add ../elsewhere/second-repo/nested/c.txt\n+four\n\
        %% end patch\n";
    let scripted = Scripted::start(vec![
        tool_turn("p1", "patch", json!({"patch": patch})),
        text_turn("done"),
    ])
    .await;
    let elsewhere = outside(&scripted, "elsewhere");
    for repository in ["first-repo", "second-repo"] {
        std::fs::create_dir_all(elsewhere.join(repository).join(".git")).unwrap();
        std::fs::create_dir_all(elsewhere.join(repository).join("nested")).unwrap();
    }
    let id = scripted.session().await;
    let settled = run_approving(&scripted, &id, "Add the files").await;
    let outside_asks: Vec<&PermissionAsk> = settled
        .asks
        .iter()
        .filter(|ask| ask.permission == "external_directory")
        .collect();
    let [ask] = outside_asks.as_slice() else {
        panic!("{:?}", asked(&settled.asks));
    };
    let ends = |values: &[String], tails: [&str; 2]| {
        values.len() == 2 && values[0].ends_with(tails[0]) && values[1].ends_with(tails[1])
    };
    assert!(
        ends(
            &ask.patterns,
            ["first-repo/nested/*", "second-repo/nested/*"]
        ),
        "{:?}",
        ask.patterns
    );
    assert!(
        ends(&ask.always, ["/first-repo/*", "/second-repo/*"]),
        "{:?}",
        ask.always
    );
    assert!(scripted.project.join("inside.txt").exists());
    scripted.finish().await;
}
