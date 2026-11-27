//! Document operations for the model, as plain async functions the shell wraps
//! as tools.
//!
//! Each answers an [`AgentOutput`]: short text written for a model (ids it can
//! pass back, the revision it worked on, what happened) and the structured
//! result for the shell. A failure is a [`DriveError`]; [`failure_text`] says
//! it in a line the model can act on (propose instead of edit, read the outline
//! again after a conflict).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use sp_protocol::ids::{ContextId, DocumentFolderId, DocumentId, DocumentRevisionId};
use sp_protocol::wire::drive::{
    CommentRequest, CreateDocumentRequest, DriveAnchor, DriveComment, DriveCreated, DriveOutline,
    DriveScope, EditMode, EditRequest,
};

use systemprompt_drive::client::{DriveClient, EditOutcome, ListQuery, ReadView};
use systemprompt_drive::error::{DriveError, NeededCapability};
use systemprompt_drive::view::{DocumentItem, document_items};

const MAX_LISTED: usize = 50;
const MAX_OUTLINE_LINES: usize = 400;

/// Text for the model plus the structured result for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOutput<T> {
    pub text: String,
    pub data: T,
}

/// An edit the model sends as a proposal: the document, its operations, the
/// revision they were written against, a line saying what the change is for,
/// and the conversation the write belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposeEdit {
    pub id: DocumentId,
    // JSON: structured edit ops are the document store's own vocabulary, passed through unchanged.
    pub ops: Vec<Value>,
    pub base_revision: Option<DocumentRevisionId>,
    pub message: Option<String>,
    pub context_id: Option<ContextId>,
}

/// A document read for the model: its name and format with its nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDocument {
    pub id: DocumentId,
    pub name: String,
    pub outline: DriveOutline,
}

pub async fn search_documents(
    client: &DriveClient,
    query: &str,
    scope: DriveScope,
) -> Result<AgentOutput<Vec<DocumentItem>>, DriveError> {
    let listing = client.search_documents(query, scope).await?;
    let items = document_items(&listing.documents);
    let heading = format!(
        "{} document(s) matching {query:?} in {}",
        items.len(),
        scope.as_str()
    );
    Ok(AgentOutput {
        text: listing_text(&heading, &items),
        data: items,
    })
}

pub async fn list_documents(
    client: &DriveClient,
    scope: DriveScope,
    folder: Option<DocumentFolderId>,
) -> Result<AgentOutput<Vec<DocumentItem>>, DriveError> {
    let listing = client
        .list_documents(&ListQuery {
            scope,
            folder,
            ..ListQuery::default()
        })
        .await?;
    let items = document_items(&listing.documents);
    let mut heading = format!("{} document(s) in {}", items.len(), scope.as_str());
    if !listing.folders.is_empty() {
        heading.push_str("; folders: ");
        let names: Vec<String> = listing
            .folders
            .iter()
            .map(|f| format!("{} (id {})", f.name, f.id))
            .collect();
        heading.push_str(&names.join(", "));
    }
    if listing.next_cursor.is_some() {
        heading.push_str("; more are available");
    }
    Ok(AgentOutput {
        text: listing_text(&heading, &items),
        data: items,
    })
}

pub async fn read_document(
    client: &DriveClient,
    id: &DocumentId,
    full_text: bool,
) -> Result<AgentOutput<ReadDocument>, DriveError> {
    let detail = client.document(id).await?;
    let view = if full_text {
        ReadView::Text
    } else {
        ReadView::Outline
    };
    let outline = client.read(id, view).await?;
    let mut text = format!(
        "{} ({}), revision {}. Each line is <node id>:<hash>| <text>; anchor comments and edits on the node id.\n",
        detail.document.name, outline.format, outline.revision_id
    );
    for line in outline.lines.iter().take(MAX_OUTLINE_LINES) {
        text.push_str(line);
        text.push('\n');
    }
    if outline.lines.len() > MAX_OUTLINE_LINES {
        text.push_str(&format!(
            "[{} more lines not shown]\n",
            outline.lines.len() - MAX_OUTLINE_LINES
        ));
    }
    Ok(AgentOutput {
        text,
        data: ReadDocument {
            id: id.clone(),
            name: detail.document.name,
            outline,
        },
    })
}

pub async fn propose_edit(
    client: &DriveClient,
    edit: ProposeEdit,
) -> Result<AgentOutput<EditOutcome>, DriveError> {
    let ProposeEdit {
        id,
        ops,
        base_revision,
        message,
        context_id,
    } = edit;
    if ops.is_empty() {
        return Err(DriveError::InvalidInput {
            what: "an edit needs at least one operation",
        });
    }
    let request = EditRequest {
        ops,
        base_revision,
        mode: Some(EditMode::Propose),
        op_id: None,
        message,
    };
    let outcome = client.edit(&id, &request, context_id.as_ref()).await?;
    let text = match &outcome {
        EditOutcome::Proposed(proposed) => format!(
            "Proposed edit {} on document {id} (revision {}); a person reviews it before it lands.",
            proposed.proposal.id, proposed.revision.id
        ),
        EditOutcome::Landed(landed) => match &landed.merged_from {
            Some(_) => format!(
                "The edit landed on document {id} as revision {}, merged onto a newer head.",
                landed.revision.id
            ),
            None => format!(
                "The edit landed on document {id} as revision {}.",
                landed.revision.id
            ),
        },
    };
    Ok(AgentOutput {
        text,
        data: outcome,
    })
}

pub async fn create_document(
    client: &DriveClient,
    template_id: &str,
    name: &str,
    folder_id: Option<DocumentFolderId>,
    context_id: Option<&ContextId>,
) -> Result<AgentOutput<DriveCreated>, DriveError> {
    let request = CreateDocumentRequest {
        template_id: template_id.to_owned(),
        name: name.to_owned(),
        folder_id,
    };
    let created = client.create_document(&request, context_id).await?;
    let text = format!(
        "Created {:?} ({}) as document {}, revision {}.",
        created.document.name, created.document.format, created.document.id, created.revision.id
    );
    Ok(AgentOutput {
        text,
        data: created,
    })
}

pub async fn add_comment(
    client: &DriveClient,
    id: &DocumentId,
    body: &str,
    anchor: DriveAnchor,
    context_id: Option<&ContextId>,
) -> Result<AgentOutput<DriveComment>, DriveError> {
    if body.trim().is_empty() {
        return Err(DriveError::InvalidInput {
            what: "a comment needs some text",
        });
    }
    let node = anchor.node.clone();
    let request = CommentRequest {
        body: body.to_owned(),
        anchor: Some(anchor),
        parent_id: None,
        mention_options: Vec::new(),
    };
    let comment = client.create_comment(id, &request, context_id).await?;
    let text = format!(
        "Commented on node {node} of document {id} (comment {}).",
        comment.id
    );
    Ok(AgentOutput {
        text,
        data: comment,
    })
}

#[must_use]
pub fn failure_text(error: &DriveError) -> String {
    match error {
        DriveError::NotFound { .. } => {
            "No such document or folder is visible to this account.".to_owned()
        },
        DriveError::AgentRefused {
            capability: NeededCapability::Edit,
            ..
        } => "The agent may only suggest on this document: send the edit as a proposal.".to_owned(),
        DriveError::AgentRefused {
            capability: NeededCapability::Person,
            ..
        } => "Only the user can do this, in the instance's own Drive.".to_owned(),
        DriveError::AgentRefused { .. } | DriveError::Forbidden { .. } => {
            "The instance does not allow this on that document.".to_owned()
        },
        DriveError::Conflict {
            error_key, head, ..
        } => {
            let mut text = String::from(
                "The document changed or the anchor is gone: read it again before retrying.",
            );
            if let Some(head) = &head.head_revision_id {
                text.push_str(&format!(" The head is now revision {head}."));
            }
            if error_key.as_deref() == Some("not_found") {
                text.push_str(" The node named is not in the current revision.");
            }
            text
        },
        DriveError::Client(client) if client.is_transient() => {
            "The instance did not answer; try again shortly.".to_owned()
        },
        DriveError::Client(client) if client.refuses_token() => {
            "The account is signed out of the instance.".to_owned()
        },
        DriveError::InvalidInput { what } => format!("The request was not sent: {what}."),
        DriveError::Cancelled => "The transfer was cancelled.".to_owned(),
        DriveError::Client(_) | DriveError::PartAltered { .. } => {
            "The Drive call failed.".to_owned()
        },
    }
}

fn listing_text(heading: &str, items: &[DocumentItem]) -> String {
    let mut text = format!("{heading}.\n");
    for item in items.iter().take(MAX_LISTED) {
        text.push_str(&format!(
            "- {} [{}] id {} updated {}\n",
            item.name, item.format, item.id, item.updated_at
        ));
    }
    if items.len() > MAX_LISTED {
        text.push_str(&format!("[{} more not shown]\n", items.len() - MAX_LISTED));
    }
    text
}
