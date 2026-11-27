//! The Drive tools that write: propose an edit, create a document and
//! comment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::{Value, json};
use sp_protocol::ids::{DocumentFolderId, DocumentId, DocumentRevisionId};
use sp_protocol::wire::drive::DriveAnchor;

use super::DriveOperation;
use crate::members::drive::agent;
use systemprompt_drive::client::DriveClient;
use systemprompt_drive::error::DriveError;
use crate::members::drive::kind::DocumentKind;

#[derive(Deserialize)]
pub(super) struct ProposeInput {
    document_id: DocumentId,
    // JSON: edit operations are the document store's own vocabulary, passed through unchanged.
    ops: Vec<Value>,
    base_revision: Option<DocumentRevisionId>,
    message: Option<String>,
}

pub(super) struct Propose;

impl DriveOperation for Propose {
    type Input = ProposeInput;
    const NAME: &'static str = "drive_propose_edit";
    const TITLE: &'static str = "Propose an edit to a Drive document";
    const DESCRIPTION: &'static str = "Propose changes to a Drive document; a person reviews them before they land. Read it first and \
        pass its revision as `base_revision`. Each op is an object named by `op`: replace_text {node, expected_hash, text}, \
        insert_after {node, text}, delete {node}, append_markdown {markdown} or str_replace {old_str, new_str}.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "required": ["document_id", "ops"], "properties": {
            "document_id": { "type": "string", "description": "The document's id." },
            "ops": { "type": "array", "items": { "type": "object" }, "description": "The edit operations, in order." },
            "base_revision": { "type": "string", "description": "The revision the ops were written against." },
            "message": { "type": "string", "description": "One line saying what the change is for." }
        }})
    }

    fn target(input: &ProposeInput) -> String {
        input.document_id.to_string()
    }

    async fn call(client: DriveClient, input: ProposeInput) -> Result<String, DriveError> {
        let ProposeInput {
            document_id,
            ops,
            base_revision,
            message,
        } = input;
        let edit = agent::ProposeEdit {
            id: document_id,
            ops,
            base_revision,
            message,
            context_id: None,
        };
        Ok(agent::propose_edit(&client, edit).await?.text)
    }
}

#[derive(Deserialize)]
pub(super) struct CreateInput {
    name: String,
    #[serde(default)]
    kind: DocumentKind,
    folder_id: Option<DocumentFolderId>,
}

pub(super) struct Create;

impl DriveOperation for Create {
    type Input = CreateInput;
    const NAME: &'static str = "drive_create";
    const TITLE: &'static str = "Create a Drive document";
    const DESCRIPTION: &'static str = "Create an empty Drive document and answer its id and revision; fill it with drive_propose_edit. `kind`: \
        markdown (default), text, html, word, spreadsheet, presentation. `folder_id` puts it in a folder.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "required": ["name"], "properties": {
            "name": { "type": "string", "description": "The document's name." },
            "kind": { "type": "string", "enum": ["markdown", "text", "html", "word", "spreadsheet", "presentation"] },
            "folder_id": { "type": "string", "description": "The folder to create it in." }
        }})
    }

    async fn call(client: DriveClient, input: CreateInput) -> Result<String, DriveError> {
        let template = input.kind.template_id();
        Ok(
            agent::create_document(&client, template, &input.name, input.folder_id, None)
                .await?
                .text,
        )
    }
}

#[derive(Deserialize)]
pub(super) struct CommentInput {
    document_id: DocumentId,
    node: String,
    quote: Option<String>,
    body: String,
}

pub(super) struct Comment;

impl DriveOperation for Comment {
    type Input = CommentInput;
    const NAME: &'static str = "drive_comment";
    const TITLE: &'static str = "Comment on a Drive document";
    const DESCRIPTION: &'static str = "Leave a comment on one node of a Drive document, the node id taken from drive_read. `quote` narrows \
        it to words inside that node. Use it to flag something for a person rather than to change the text.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "required": ["document_id", "node", "body"], "properties": {
            "document_id": { "type": "string", "description": "The document's id." },
            "node": { "type": "string", "description": "The node id the comment is pinned to." },
            "quote": { "type": "string", "description": "Words inside the node the comment is about." },
            "body": { "type": "string", "description": "The comment." }
        }})
    }

    fn target(input: &CommentInput) -> String {
        input.document_id.to_string()
    }

    async fn call(client: DriveClient, input: CommentInput) -> Result<String, DriveError> {
        let anchor = DriveAnchor {
            node: input.node,
            find: input.quote,
        };
        Ok(
            agent::add_comment(&client, &input.document_id, &input.body, anchor, None)
                .await?
                .text,
        )
    }
}
