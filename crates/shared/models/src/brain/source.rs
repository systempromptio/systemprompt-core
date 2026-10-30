//! Where a fact came from, and the resource whose readability gates it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AgentRunId, DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, SessionId,
    UserId,
};

use super::envelope::Pathway;
use crate::notifications::SubjectRef;

pub const DOCUMENT_SUBJECT_KIND: &str = "document";
pub const EXTERNAL_SUBJECT_PREFIX: &str = "external:";

/// The passage of a document revision a fact cites: the node, the first
/// eight hex characters of its content hash, the quoted text and which
/// occurrence of it within the node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub node_id: DocumentNodeId,
    pub hash8: String,
    pub find: String,
    #[serde(default)]
    pub nth: u32,
}

/// The typed origin of a fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FactSource {
    Document {
        document_id: DocumentId,
        revision_id: DocumentRevisionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        span: Option<SourceSpan>,
    },
    Comment {
        document_id: DocumentId,
        comment_id: DocumentCommentId,
    },
    Session {
        session_id: SessionId,
        owner: UserId,
    },
    EnteredBy {
        user_id: UserId,
    },
    External {
        system: String,
        reference: String,
    },
    Run {
        run_id: AgentRunId,
    },
    Aggregate {
        pathway: Pathway,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    },
}

/// The class of a [`FactSource`], as importance weights name it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceClass {
    Document,
    Comment,
    Session,
    EnteredBy,
    External,
    Run,
    Aggregate,
}

impl SourceClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Comment => "comment",
            Self::Session => "session",
            Self::EnteredBy => "entered_by",
            Self::External => "external",
            Self::Run => "run",
            Self::Aggregate => "aggregate",
        }
    }
}

impl FactSource {
    #[must_use]
    pub const fn class(&self) -> SourceClass {
        match self {
            Self::Document { .. } => SourceClass::Document,
            Self::Comment { .. } => SourceClass::Comment,
            Self::Session { .. } => SourceClass::Session,
            Self::EnteredBy { .. } => SourceClass::EnteredBy,
            Self::External { .. } => SourceClass::External,
            Self::Run { .. } => SourceClass::Run,
            Self::Aggregate { .. } => SourceClass::Aggregate,
        }
    }

    #[must_use]
    pub const fn document_id(&self) -> Option<&DocumentId> {
        match self {
            Self::Document { document_id, .. } | Self::Comment { document_id, .. } => {
                Some(document_id)
            },
            _ => None,
        }
    }

    #[must_use]
    pub fn visibility_subject(&self) -> Option<SubjectRef> {
        match self {
            Self::Document { document_id, .. } | Self::Comment { document_id, .. } => {
                Some(SubjectRef::new(DOCUMENT_SUBJECT_KIND, document_id.as_str()))
            },
            Self::External { system, reference } => Some(SubjectRef::new(
                format!("{EXTERNAL_SUBJECT_PREFIX}{system}"),
                reference.clone(),
            )),
            Self::Session { .. }
            | Self::EnteredBy { .. }
            | Self::Run { .. }
            | Self::Aggregate { .. } => None,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Document {
                document_id,
                revision_id,
                ..
            } => format!("doc:{document_id}@{revision_id}"),
            Self::Comment { comment_id, .. } => format!("comment:{comment_id}"),
            Self::Session { session_id, .. } => format!("session:{session_id}"),
            Self::EnteredBy { user_id } => format!("entered:{user_id}"),
            Self::External { system, reference } => format!("{system}:{reference}"),
            Self::Run { run_id } => format!("run:{run_id}"),
            Self::Aggregate { pathway, .. } => format!("aggregate:{}", pathway.as_str()),
        }
    }
}
