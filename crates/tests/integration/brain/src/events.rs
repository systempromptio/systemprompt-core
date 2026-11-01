use chrono::Utc;
use systemprompt_identifiers::{
    ContentHash, DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId,
    DocumentRevisionId, FactKindId, UserId,
};
use systemprompt_models::brain::{
    BrainEvent, BrainEventKind, CommentAnchorRef, CommentResolved, EnteredFact, EnteredStatus,
    FactScope, RevisionLanded, SourceNode, SourceText, SubjectKey,
};
use systemprompt_models::documents::{ActorKind, RevisionOriginKind};

pub(crate) const PRODUCT: &str = "product";

pub(crate) struct Landed {
    pub(crate) document: DocumentId,
    pub(crate) revision: DocumentRevisionId,
    pub(crate) folder: Option<DocumentFolderId>,
}

impl Landed {
    pub(crate) fn new() -> Self {
        Self {
            document: DocumentId::generate(),
            revision: DocumentRevisionId::generate(),
            folder: None,
        }
    }

    pub(crate) fn next_revision(&self) -> Self {
        Self {
            document: self.document.clone(),
            revision: DocumentRevisionId::generate(),
            folder: self.folder.clone(),
        }
    }

    pub(crate) fn in_folder(mut self, folder: DocumentFolderId) -> Self {
        self.folder = Some(folder);
        self
    }

    pub(crate) fn event(&self, author: &UserId) -> BrainEvent {
        let payload = RevisionLanded {
            document_id: self.document.clone(),
            revision_id: self.revision.clone(),
            parent_revision_id: None,
            actor_kind: ActorKind::User,
            origin_kind: RevisionOriginKind::Api,
            author: author.clone(),
            session_id: None,
            context_id: None,
            trace_id: None,
            task_id: None,
            on_behalf_of: None,
            folder_id: self.folder.clone(),
            content_sha256: ContentHash::try_new("a".repeat(64)).expect("content hash"),
        };
        BrainEvent::new(
            BrainEventKind::revision_landed(),
            author.clone(),
            Utc::now(),
        )
        .with_payload(serde_json::to_value(payload).expect("payload"))
    }
}

pub(crate) fn comment_resolved(
    resolver: &UserId,
    document: &DocumentId,
    comment: &DocumentCommentId,
) -> BrainEvent {
    let payload = CommentResolved {
        document_id: document.clone(),
        comment_id: comment.clone(),
        comment_author: resolver.clone(),
        resolved_by: resolver.clone(),
        anchor: CommentAnchorRef {
            node_id: DocumentNodeId::try_new("n1").expect("node id"),
            hash8: None,
            find: None,
            nth: None,
        },
        head_revision_id: None,
        answering_revision_id: None,
        folder_id: None,
    };
    BrainEvent::new(
        BrainEventKind::comment_resolved(),
        resolver.clone(),
        Utc::now(),
    )
    .with_payload(serde_json::to_value(payload).expect("payload"))
}

pub(crate) fn entered(
    who: &UserId,
    kind: FactKindId,
    product: &str,
    fields: serde_json::Value,
    scope: FactScope,
) -> BrainEvent {
    let serde_json::Value::Object(fields) = fields else {
        panic!("entered fields must be an object");
    };
    EnteredFact {
        kind,
        subject: SubjectKey::new(PRODUCT, product),
        fields,
        scope,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
    }
    .to_event(who.clone(), Utc::now())
    .expect("entered event")
}

pub(crate) fn price(product: &str, region: &str, price: &str) -> serde_json::Value {
    serde_json::json!({ "subject": product, "price": price, "region": region })
}

pub(crate) fn source_text(body: &str) -> SourceText {
    SourceText {
        text: body.to_owned(),
        nodes: vec![SourceNode {
            node_id: DocumentNodeId::try_new("n1").expect("node id"),
            hash8: "deadbeef".to_owned(),
            text: body.to_owned(),
        }],
    }
}
