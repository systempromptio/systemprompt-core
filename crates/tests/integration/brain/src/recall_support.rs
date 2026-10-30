use systemprompt_extension::brain::{
    MemoryRecall, RecallBlock, RecallConversation, RecallOutcome, RecallPrincipal, RecallRequest,
    ReuseHint,
};
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::origin::ClientKind;

use crate::harness::Harness;

pub(crate) fn principal(user: &UserId, project: Option<&str>) -> RecallPrincipal {
    RecallPrincipal {
        user_id: user.clone(),
        client: ClientKind::ClaudeCode,
        actor: RequestActor::Human,
        project: project.map(str::to_owned),
        client_id: None,
    }
}

pub(crate) fn request(principal: RecallPrincipal, hints: &str) -> RecallRequest {
    RecallRequest::new(principal, RecallConversation::default())
        .with_subject_hints(hints)
        .with_budget_tokens(800)
        .with_reuse(ReuseHint::NewTurn)
}

pub(crate) async fn recall(h: &Harness, user: &UserId, hints: &str) -> RecallOutcome {
    h.recall()
        .recall(&request(principal(user, None), hints))
        .await
        .expect("recall")
}

pub(crate) fn injected(outcome: &RecallOutcome) -> Option<&RecallBlock> {
    match outcome {
        RecallOutcome::Injected(block) => Some(block),
        RecallOutcome::Skipped { .. } => None,
    }
}

pub(crate) fn ids(outcome: &RecallOutcome) -> Vec<BrainFactId> {
    injected(outcome)
        .map(|block| block.facts.iter().map(|f| f.id.clone()).collect())
        .unwrap_or_default()
}
