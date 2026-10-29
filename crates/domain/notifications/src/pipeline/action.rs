//! [`ActionPromptBuilder`]: the instruction a notification carries for the
//! person or agent reading it, worded per kind, with a deep link when the
//! event concerns a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId};
use systemprompt_models::notifications::{ActionPrompt, NotificationEvent, NotificationKind};

/// Builds the [`ActionPrompt`] for one event.
#[derive(Debug, Clone, Copy)]
pub struct ActionPromptBuilder<'a> {
    event: &'a NotificationEvent,
}

impl<'a> ActionPromptBuilder<'a> {
    #[must_use]
    pub const fn new(event: &'a NotificationEvent) -> Self {
        Self { event }
    }

    #[must_use]
    pub fn build(self) -> ActionPrompt {
        let document_id = self
            .subject_id("document")
            .and_then(|id| DocumentId::try_new(id).ok());
        let comment_id = self
            .subject_id("comment")
            .and_then(|id| DocumentCommentId::try_new(id).ok());
        let open_url = document_id.as_ref().map(|doc| {
            comment_id.as_ref().map_or_else(
                || format!("/admin/drive/{doc}"),
                |comment| format!("/admin/drive/{doc}?comment={comment}"),
            )
        });
        ActionPrompt {
            prompt: self.prompt(document_id.as_ref()),
            open_url,
            document_id,
            comment_id,
            kind: self.event.kind.clone(),
            actor: self.event.actor.clone(),
        }
    }

    fn subject_id(&self, kind: &str) -> Option<&'a str> {
        self.event
            .subjects
            .iter()
            .find(|s| s.kind == kind)
            .map(|s| s.id.as_str())
    }

    fn prompt(&self, document: Option<&DocumentId>) -> String {
        let actor = &self.event.actor;
        let kind = self.event.kind.as_str();
        let document = document.map_or_else(|| "(unknown)".to_owned(), ToString::to_string);
        match kind {
            NotificationKind::MENTIONED => format!(
                "{actor} mentioned you in a comment on document {document}. Fetch the document \
                 and the comment thread through the documents MCP server (`documents_comment` \
                 {{action: list}}), read the anchor, then reply with `documents_comment` and \
                 mark the item read with `documents_inbox`."
            ),
            NotificationKind::SHARED => format!(
                "{actor} shared document {document} with you. Open it through the documents MCP \
                 server (`documents_read`), then mark the item read with `documents_inbox`."
            ),
            NotificationKind::STREAM_PUBLISHED => {
                let stream = self
                    .event
                    .stream
                    .as_ref()
                    .map_or_else(String::new, |s| format!(" to stream {s}"));
                format!(
                    "{actor} published{stream}. Read the item, act on it if it asks for \
                     something, then mark it read with `documents_inbox`."
                )
            },
            other => {
                let subject = self
                    .event
                    .subjects
                    .first()
                    .map_or_else(String::new, |s| format!(" on {}:{}", s.kind, s.id));
                format!(
                    "{actor} raised {other}{subject}. Review it, then mark the item read with \
                     `documents_inbox`."
                )
            },
        }
    }
}
