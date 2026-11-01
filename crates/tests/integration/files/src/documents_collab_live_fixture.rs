use systemprompt_files::documents::collab::{CollabError, JoinMode};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::service::{LiveEdit, LiveEditRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::{
    ContentAs, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};

use crate::documents_collab::{Collab, collab, html_document};
use crate::documents_service_fixture::trace;

pub(crate) async fn live_collab() -> Collab {
    let c = collab().await;
    c.documents.attach_collab(&c.collab);
    c
}

pub(crate) async fn open_session(c: &Collab, who: &AuthenticatedUser, document: &DocumentId) {
    c.collab
        .join(who, document, JoinMode::Edit, &trace())
        .await
        .expect("join");
}

pub(crate) async fn live(c: &Collab, who: &AuthenticatedUser, document: &DocumentId) -> Projection {
    c.documents
        .live_read(who, document, &trace())
        .await
        .expect("live read")
        .expect("open session")
        .1
}

pub(crate) fn texts(projection: &Projection) -> Vec<String> {
    projection
        .nodes()
        .filter_map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect()
}

pub(crate) fn node_with<'a>(projection: &'a Projection, text: &str) -> &'a ProjectionNode {
    projection
        .nodes()
        .find(|node| node.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no node holds {text:?}: {:?}", texts(projection)))
}

pub(crate) async fn edit(
    c: &Collab,
    who: &AuthenticatedUser,
    document: &DocumentId,
    ops: Vec<EditOp>,
) -> Result<Option<LiveEdit>, CollabError> {
    c.collab
        .edit_live(
            who,
            LiveEditRequest {
                document_id: document.clone(),
                ops,
                media: Media::new(),
            },
            &trace(),
        )
        .await
}

pub(crate) fn id_of(node: &ProjectionNode) -> NodeTarget {
    NodeTarget::Id(node.id.clone())
}

pub(crate) fn insert(
    after: &ProjectionNode,
    kind: NodeKind,
    text: &str,
    content_as: ContentAs,
) -> EditOp {
    EditOp::InsertAfter {
        node: id_of(after),
        kind,
        text: text.to_owned(),
        content_as,
    }
}

pub(crate) async fn seeded(c: &Collab, who: &AuthenticatedUser) -> DocumentId {
    let document = html_document(c, who).await;
    open_session(c, who, &document).await;
    let head = live(c, who, &document).await;
    let first = head.nodes().next().expect("a block").clone();
    edit(
        c,
        who,
        &document,
        vec![insert(
            &first,
            NodeKind::Paragraph,
            "alpha line",
            ContentAs::Text,
        )],
    )
    .await
    .expect("seed")
    .expect("live");
    document
}
