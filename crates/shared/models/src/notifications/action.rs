//! The action a notification prompts its reader to take.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, UserId};

use super::kind::NotificationKind;

/// What the reader (a person or an agent) is asked to do, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActionPrompt {
    pub prompt: String,
    pub open_url: Option<String>,
    pub document_id: Option<DocumentId>,
    pub comment_id: Option<DocumentCommentId>,
    pub kind: NotificationKind,
    pub actor: UserId,
}
