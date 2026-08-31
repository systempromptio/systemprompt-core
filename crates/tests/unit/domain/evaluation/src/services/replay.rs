//! `ReplayService::replay` — re-running a stored case with a repair hint.
//!
//! The hint's *position* is the whole point of this service. It is injected
//! immediately before the last user turn, so the model reads the correction
//! and then the question it applies to. Appended at the end it would arrive
//! after the question and read as a new instruction; placed at the front it
//! would be buried under the transcript. These assert the position rather
//! than merely that the hint appears somewhere.
//!
//! The mock records the `AiRequest` it was handed, which is what makes the
//! assembled message list observable without a live provider.

use std::sync::Arc;

use systemprompt_evaluation::models::{CanonicalMessage, CanonicalPrompt};
use systemprompt_evaluation::services::ReplayService;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::ai::{AiResponse, DynAiProvider, MessageRole};
use systemprompt_test_mocks::{MockAiCall, MockAiProvider};

const HINT: &str = "cite the source next time";

fn msg(role: &str, content: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: role.to_owned(),
        content: content.to_owned(),
    }
}

fn prompt(messages: Vec<CanonicalMessage>, system_prompt: Option<&str>) -> CanonicalPrompt {
    CanonicalPrompt {
        messages,
        system_prompt: system_prompt.map(ToOwned::to_owned),
        offered_tools: None,
        provider: "mock".to_owned(),
        model: "mock-replay".to_owned(),
    }
}

fn ai_response() -> AiResponse {
    let mut resp = AiResponse::default();
    resp.request_id = uuid::Uuid::new_v4();
    resp.content = "replayed answer".to_owned();
    resp.provider = "mock".to_owned();
    resp.model = "mock-replay".to_owned();
    resp
}

fn service() -> (ReplayService, Arc<MockAiProvider>) {
    let mock = Arc::new(
        MockAiProvider::builder()
            .with_generate_response(Ok(ai_response()))
            .build(),
    );
    let ai: DynAiProvider = Arc::clone(&mock) as DynAiProvider;
    (
        ReplayService::new(ai, UserId::new("replay-test-user"), ContextId::generate()),
        mock,
    )
}

fn sent_messages(mock: &MockAiProvider) -> Vec<(MessageRole, String)> {
    mock.calls()
        .into_iter()
        .find_map(|call| match call {
            MockAiCall::Generate { request } => Some(request),
            _ => None,
        })
        .expect("the provider should have been asked to generate")
        .messages
        .into_iter()
        .map(|m| (m.role, m.content))
        .collect()
}

#[tokio::test]
async fn an_empty_prompt_is_refused() {
    let (service, _mock) = service();

    let err = service
        .replay(&prompt(Vec::new(), None), HINT)
        .await
        .expect_err("a prompt with no messages cannot be replayed");

    assert!(
        format!("{err}").contains("no messages"),
        "the error should say the prompt was empty: {err}"
    );
}

// Why: without a user turn there is nothing for the correction to attach to,
// so replaying would send the hint into a transcript it cannot modify.
#[tokio::test]
async fn a_prompt_with_no_user_turn_is_refused() {
    let (service, _mock) = service();

    let err = service
        .replay(
            &prompt(
                vec![msg("system", "be terse"), msg("assistant", "hello")],
                None,
            ),
            HINT,
        )
        .await
        .expect_err("a prompt with no user turn cannot be replayed");

    assert!(
        format!("{err}").contains("no user turn"),
        "the error should say there was no user turn: {err}"
    );
}

#[tokio::test]
async fn the_hint_lands_immediately_before_the_last_user_turn() {
    let (service, mock) = service();

    service
        .replay(
            &prompt(
                vec![
                    msg("user", "first question"),
                    msg("assistant", "first answer"),
                    msg("user", "second question"),
                ],
                None,
            ),
            HINT,
        )
        .await
        .expect("replay");

    let sent = sent_messages(&mock);
    let hint_at = sent
        .iter()
        .position(|(_, c)| c.contains(HINT))
        .expect("the repair hint should be in the sent messages");
    let last_user_at = sent
        .iter()
        .rposition(|(role, _)| *role == MessageRole::User)
        .expect("there should be a user turn");

    assert_eq!(
        hint_at + 1,
        last_user_at,
        "the hint must sit directly before the LAST user turn, not the first \
         and not at the end; got {sent:?}"
    );
    assert_eq!(
        sent[last_user_at].1, "second question",
        "the hint should precede the most recent question"
    );
}

#[tokio::test]
async fn canonical_roles_map_onto_provider_roles() {
    let (service, mock) = service();

    service
        .replay(
            &prompt(
                vec![
                    msg("system", "be terse"),
                    msg("assistant", "prior answer"),
                    msg("user", "the question"),
                ],
                None,
            ),
            HINT,
        )
        .await
        .expect("replay");

    let sent = sent_messages(&mock);
    assert!(
        sent.iter()
            .any(|(r, c)| *r == MessageRole::System && c == "be terse")
    );
    assert!(
        sent.iter()
            .any(|(r, c)| *r == MessageRole::Assistant && c == "prior answer")
    );
    assert!(
        sent.iter()
            .any(|(r, c)| *r == MessageRole::User && c == "the question")
    );
}

// Why: an unrecognised role must become User rather than be dropped. Dropping
// it would silently replay a shorter conversation than the one that failed.
#[tokio::test]
async fn an_unrecognised_role_is_carried_as_a_user_turn() {
    let (service, mock) = service();

    service
        .replay(
            &prompt(
                vec![msg("tool", "tool output"), msg("user", "the question")],
                None,
            ),
            HINT,
        )
        .await
        .expect("replay");

    let sent = sent_messages(&mock);
    assert!(
        sent.iter()
            .any(|(r, c)| *r == MessageRole::User && c == "tool output"),
        "an unknown role should be carried as User, not dropped: {sent:?}"
    );
}

#[tokio::test]
async fn a_provider_failure_surfaces_as_an_ai_error() {
    let mock = Arc::new(
        MockAiProvider::builder()
            .with_generate_error(anyhow::anyhow!("replay upstream refused"))
            .build(),
    );
    let ai: DynAiProvider = mock as DynAiProvider;
    let service = ReplayService::new(ai, UserId::new("replay-test-user"), ContextId::generate());

    let err = service
        .replay(&prompt(vec![msg("user", "q")], None), HINT)
        .await
        .expect_err("a provider failure must not produce a replay");

    assert!(
        format!("{err}").contains("replay upstream refused"),
        "the provider's own message should survive: {err}"
    );
}
