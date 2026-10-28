//! A harness whose agent access policy the test sets, and the calls an
//! agent makes through it.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{
    AgentAccessPolicy, AgentActor, AgentCap, AgentCapability, AgentRefusal, DocumentActor,
    DocumentContext, act_as,
};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;
use systemprompt_test_fixtures::test_db_pool;
use tempfile::TempDir;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, api_author, create, harness_with_policy, trace,
};

pub(crate) struct FixedPolicy {
    pub(crate) cap: AgentCap,
    pub(crate) allows: Vec<AgentCapability>,
}

#[async_trait]
impl AgentAccessPolicy for FixedPolicy {
    fn name(&self) -> &'static str {
        "fixed"
    }

    async fn cap(&self, _doc: &DocumentContext, _agent: &AgentActor) -> AgentCap {
        self.cap
    }

    async fn allows(
        &self,
        capability: AgentCapability,
        _doc: &DocumentContext,
        _agent: &AgentActor,
    ) -> bool {
        self.allows.contains(&capability)
    }
}

pub(crate) async fn agent_harness(
    cap: AgentCap,
    allows: Vec<AgentCapability>,
    directory: RoleDirectory,
    config: DocumentsConfig,
) -> Harness {
    harness_with_policy(
        test_db_pool().await,
        (allow(), GroupSource::new(&[]), directory),
        Arc::new(TempDir::new().expect("tempdir")),
        config,
        Some(Arc::new(FixedPolicy { cap, allows })),
    )
}

pub(crate) fn agent() -> DocumentActor {
    DocumentActor::agent("claude-code", true)
}

pub(crate) async fn document(h: &Harness, owner: &AuthenticatedUser) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

/// Rewrites the document's text to `text` with one `str_replace` at the
/// head (not `set_text`, which agents meet as a whole-content replace),
/// made as `who` by the running actor.
pub(crate) async fn edit(
    h: &Harness,
    who: &AuthenticatedUser,
    id: &DocumentId,
    text: &str,
) -> Result<CommitOutcome, DocumentsError> {
    let (head, current) = h.service.plain_text(who, id, None, &trace()).await?;
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: Some(head.id),
        body: CommitBody::Ops(vec![EditOp::StrReplace {
            old_str: current.trim().to_owned(),
            new_str: text.trim().to_owned(),
        }]),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    };
    h.service.commit(who, request, &trace()).await
}

pub(crate) async fn as_agent<F: std::future::Future>(work: F) -> F::Output {
    act_as(agent(), work).await
}

pub(crate) const fn agent_refusal(error: &DocumentsError) -> Option<&AgentRefusal> {
    match error {
        DocumentsError::Agent(refusal) => Some(refusal),
        _ => None,
    }
}
