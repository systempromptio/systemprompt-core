//! How a step settles what the model asked for: every tool call of a step is
//! run and recorded before the next request, and a step the output limit cut
//! off runs none of the calls it was cut off in.

use serde_json::json;
use systemprompt_engine::run::Delivery;
use systemprompt_engine::store::{FinishReason, Message, ToolState};
use systemprompt_scripted_instance::{Respond, StopReason, ToolCall as Call, Turn};

use crate::support::{Scripted, read_turn, settle, text_turn, tool_states};

fn read_call(id: &str) -> Respond {
    Respond::ToolCall(Call {
        id: id.to_owned(),
        name: "read".to_owned(),
        input: json!({"path": "notes.txt"}),
        argument_chunks: Vec::new(),
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn every_tool_call_of_a_step_is_settled_before_the_one_continuation() {
    let mut both = read_turn("first");
    both.respond.push(read_call("second"));
    let scripted = Scripted::start(vec![both, text_turn("Read twice.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read the notes twice", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);

    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(
            states.as_slice(),
            [ToolState::Completed { .. }, ToolState::Completed { .. }]
        ),
        "{states:?}"
    );
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2, "both results go back in one continuation");
    let second = bodies[1].to_string();
    assert!(
        second.contains("\"first\"") && second.contains("\"second\""),
        "{second}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_step_cut_off_by_the_output_limit_runs_none_of_its_tool_calls() {
    let cut = Turn {
        stop: Some(StopReason::MaxTokens),
        ..read_turn("cut")
    };
    let scripted = Scripted::start(vec![cut, text_turn("unused")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;

    let messages = scripted.messages(&id).await;
    let states = tool_states(&messages);
    assert!(
        !states
            .iter()
            .any(|state| matches!(state, ToolState::Completed { .. })),
        "{states:?}"
    );
    assert_eq!(
        scripted.model_bodies().len(),
        1,
        "the turn ends at the limit"
    );
    let finish = messages
        .iter()
        .rev()
        .find_map(|entry| match &entry.message {
            Message::Assistant(reply) => reply.finish,
            Message::User(_) => None,
        });
    assert_eq!(finish, Some(FinishReason::Length));
    scripted.finish().await;
}
