//! The engine's own tools in a run: the question tool, the skill tool and
//! the skills listing, and tools withheld by permission rules.

use serde_json::json;
use systemprompt_engine::run::{Delivery, EngineEvent};
use systemprompt_engine::store::ToolState;

use crate::support::{Scripted, Setup, settle, text_turn, tool_states, tool_turn, until};

fn with_skill(scripted: &Scripted) {
    let dir = scripted.project.join(".harness/skills/deploy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships a release\n---\nRun the release checklist step by step.\n",
    )
    .unwrap();
}

fn tool_names(body: &serde_json::Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| t["name"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn question_turn() -> systemprompt_scripted_instance::Turn {
    tool_turn(
        "q1",
        "question",
        json!({"questions": [{"question": "Which database?", "header": "Database", "options": [
            {"label": "Postgres", "description": "relational"},
            {"label": "SQLite", "description": "embedded"}
        ]}]}),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn the_users_answers_to_a_question_are_returned_to_the_model() {
    let scripted = Scripted::start(vec![question_turn(), text_turn("Using SQLite.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Pick a database", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::QuestionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::QuestionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(ask.questions[0].options.len(), 2);
    assert!(
        scripted
            .engine
            .answer_question(&ask.id, Some(vec![vec!["SQLite".to_owned()]]))
    );
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1].to_string().contains("SQLite"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dismissed_question_gives_the_model_no_answer() {
    let scripted = Scripted::start(vec![question_turn(), text_turn("No answer then.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Pick a database", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::QuestionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::QuestionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    scripted.engine.answer_question(&ask.id, None);
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Error { .. }]),
        "{states:?}"
    );
    assert!(
        scripted
            .model_bodies()
            .iter()
            .skip(1)
            .all(|body| !body.to_string().contains("Postgres\""))
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_question_tool_is_offered_only_when_permitted() {
    let scripted = Scripted::with(
        vec![text_turn("hi")],
        Setup {
            settings: Some(r#"{"permission": {"question": "deny"}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert!(!tool_names(&scripted.model_bodies()[0]).contains(&"question".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_skill_tool_loads_the_chosen_skill_for_the_model() {
    let scripted = Scripted::start(vec![
        tool_turn("s1", "skill", json!({"name": "deploy"})),
        text_turn("Following it."),
    ])
    .await;
    with_skill(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Ship it", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    let system = bodies[0]["system"].to_string();
    assert!(system.contains("deploy") && system.contains("Ships a release"));
    assert!(
        bodies[1]
            .to_string()
            .contains("Run the release checklist step by step.")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_skill_blocked_by_the_rules_is_refused_and_its_content_never_loaded() {
    let scripted = Scripted::with(
        vec![
            tool_turn("s1", "skill", json!({"name": "deploy"})),
            text_turn("Blocked."),
        ],
        Setup {
            settings: Some(r#"{"permission": {"skill": {"deploy": "deny"}}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    with_skill(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Ship it", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Error { .. }]),
        "{states:?}"
    );
    assert!(
        scripted
            .model_bodies()
            .iter()
            .all(|b| !b.to_string().contains("Run the release checklist"))
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_denied_every_skill_gets_no_skills_section() {
    let scripted = Scripted::with(
        vec![text_turn("hi")],
        Setup {
            settings: Some(r#"{"permission": {"skill": "deny"}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    with_skill(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let body = &scripted.model_bodies()[0];
    assert!(!body["system"].to_string().contains("Ships a release"));
    assert!(!tool_names(body).contains(&"skill".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_skill_allowed_again_after_a_blanket_denial_stays_listed() {
    let scripted = Scripted::with(
        vec![text_turn("hi")],
        Setup {
            settings: Some(
                r#"{"permission": {"skill": {"*": "deny", "deploy": "allow"}}}"#.to_owned(),
            ),
            ..Setup::default()
        },
    )
    .await;
    with_skill(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert!(
        scripted.model_bodies()[0]["system"]
            .to_string()
            .contains("Ships a release")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_denied_outright_by_the_rules_is_not_offered() {
    let scripted = Scripted::with(
        vec![text_turn("hi")],
        Setup {
            settings: Some(
                r#"{"permission": {"fetch": "deny", "shell": {"*": "deny", "git *": "allow"}}}"#
                    .to_owned(),
            ),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let names = tool_names(&scripted.model_bodies()[0]);
    assert!(!names.iter().any(|name| name == "fetch"), "{names:?}");
    assert!(names.iter().any(|name| name == "read"), "{names:?}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_custom_command_applies_its_agent_before_its_prompt_is_sent() {
    let scripted = Scripted::start(vec![text_turn("planned")]).await;
    let dir = scripted.project.join(".harness/commands");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("review.md"),
        "---\nagent: plan\n---\nReview $ARGUMENTS\n",
    )
    .unwrap();
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let input = systemprompt_engine::run::CommandInput {
        session_id: id.clone(),
        directory: scripted.project.clone(),
        name: "review".to_owned(),
        arguments: "the diff".to_owned(),
        agent: None,
        model: None,
        delivery: Delivery::Steer,
    };
    scripted.engine.command(input).await.unwrap();
    settle(&mut events, &id).await;
    let messages = scripted.messages(&id).await;
    let user = messages.iter().find_map(|m| m.message.as_user()).unwrap();
    assert_eq!(user.agent, "plan");
    assert_eq!(crate::support::texts(&messages[0]), "Review the diff");
    assert_eq!(
        scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .agent
            .as_deref(),
        Some("plan")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the model block sits in the environment section near the end, not right after the agent prompt"]
async fn the_system_prompt_names_the_model_directly_after_the_agent_prompt() {
    let scripted = Scripted::start(vec![text_turn("hi")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let system = &scripted.model_bodies()[0]["system"];
    let blocks: Vec<String> = system
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .map(|b| b["text"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_else(|| vec![system.as_str().unwrap_or_default().to_owned()]);
    assert!(
        blocks.len() > 1 && blocks[1].contains(crate::support::MODEL),
        "{:?}",
        blocks
            .iter()
            .map(|b| b.chars().take(60).collect::<String>())
            .collect::<Vec<_>>()
    );
    scripted.finish().await;
}
