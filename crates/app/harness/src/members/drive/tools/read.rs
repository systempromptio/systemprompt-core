//! The Drive tools that read: search, list and read a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::{Value, json};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::documents::client::DriveScope;

use super::{DriveOperation, SCOPES};
use crate::members::drive::agent;
use systemprompt_drive::client::DriveClient;
use systemprompt_drive::error::DriveError;

#[derive(Deserialize)]
pub(super) struct SearchInput {
    query: String,
    scope: Option<DriveScope>,
}

pub(super) struct Search;

impl DriveOperation for Search {
    type Input = SearchInput;
    const NAME: &'static str = "drive_search";
    const TITLE: &'static str = "Search the Drive";
    const DESCRIPTION: &'static str = "Find documents in the user's organisation Drive by words in their name or text: one line each with name, \
        format, id and last change. Read one with drive_read before quoting it. `scope`: mine (default), shared, recent.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "required": ["query"], "properties": {
            "query": { "type": "string", "description": "Words to look for." },
            "scope": { "type": "string", "enum": SCOPES, "description": "Whose documents to search." }
        }})
    }

    async fn call(client: DriveClient, input: SearchInput) -> Result<String, DriveError> {
        Ok(agent::search_documents(
            &client,
            &input.query,
            input.scope.unwrap_or(DriveScope::Mine),
        )
        .await?
        .text)
    }
}

#[derive(Deserialize)]
pub(super) struct ListInput {
    scope: Option<DriveScope>,
    folder_id: Option<DocumentFolderId>,
}

pub(super) struct List;

impl DriveOperation for List {
    type Input = ListInput;
    const NAME: &'static str = "drive_list";
    const TITLE: &'static str = "List Drive documents";
    const DESCRIPTION: &'static str = "List the documents of the user's Drive: their own (`scope` mine, the default), those shared with \
        them, or recently opened ones; with `folder_id`, one folder's documents. Folder names and ids come first.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "properties": {
            "scope": { "type": "string", "enum": SCOPES, "description": "Which documents to list." },
            "folder_id": { "type": "string", "description": "Only this folder's documents." }
        }})
    }

    async fn call(client: DriveClient, input: ListInput) -> Result<String, DriveError> {
        let scope = if input.folder_id.is_some() {
            DriveScope::Folder
        } else {
            input.scope.unwrap_or(DriveScope::Mine)
        };
        Ok(agent::list_documents(&client, scope, input.folder_id)
            .await?
            .text)
    }
}

#[derive(Deserialize)]
pub(super) struct ReadInput {
    document_id: DocumentId,
    #[serde(default)]
    full_text: bool,
}

pub(super) struct Read;

impl DriveOperation for Read {
    type Input = ReadInput;
    const NAME: &'static str = "drive_read";
    const TITLE: &'static str = "Read a Drive document";
    const DESCRIPTION: &'static str = "Read a Drive document: its revision, then one node per line as `<node id>:<hash>| <text>`, the ids that \
        drive_comment and drive_propose_edit name. Set `full_text` for the whole text rather than the outline.";

    // JSON: the input schema, as JSON Schema.
    fn schema() -> Value {
        json!({ "type": "object", "required": ["document_id"], "properties": {
            "document_id": { "type": "string", "description": "The document's id." },
            "full_text": { "type": "boolean", "description": "Return the whole text rather than the outline." }
        }})
    }

    fn target(input: &ReadInput) -> String {
        input.document_id.to_string()
    }

    async fn call(client: DriveClient, input: ReadInput) -> Result<String, DriveError> {
        Ok(
            agent::read_document(&client, &input.document_id, input.full_text)
                .await?
                .text,
        )
    }
}
