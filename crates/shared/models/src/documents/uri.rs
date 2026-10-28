//! The `document://<id>[@<revision>][#<node>]` reference agents quote to
//! name a precise location in a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentUriError {
    #[error("document URI must start with `document://`")]
    MissingScheme,
    #[error("document URI has an invalid {part}")]
    InvalidPart {
        part: &'static str,
        #[source]
        source: IdValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DocumentUri {
    pub document_id: DocumentId,
    pub revision_id: Option<DocumentRevisionId>,
    pub node: Option<DocumentNodeId>,
}

impl DocumentUri {
    pub const SCHEME: &'static str = "document://";

    #[must_use]
    pub const fn new(document_id: DocumentId) -> Self {
        Self {
            document_id,
            revision_id: None,
            node: None,
        }
    }

    #[must_use]
    pub fn at_revision(mut self, revision_id: DocumentRevisionId) -> Self {
        self.revision_id = Some(revision_id);
        self
    }

    #[must_use]
    pub fn at_node(mut self, node: DocumentNodeId) -> Self {
        self.node = Some(node);
        self
    }
}

impl fmt::Display for DocumentUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::SCHEME, self.document_id.as_str())?;
        if let Some(revision) = &self.revision_id {
            write!(f, "@{}", revision.as_str())?;
        }
        if let Some(node) = &self.node {
            write!(f, "#{}", node.as_str())?;
        }
        Ok(())
    }
}

impl FromStr for DocumentUri {
    type Err = DocumentUriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s
            .strip_prefix(Self::SCHEME)
            .ok_or(DocumentUriError::MissingScheme)?;
        let (rest, node) = match rest.split_once('#') {
            Some((head, node)) => (head, Some(node)),
            None => (rest, None),
        };
        let (document, revision) = match rest.split_once('@') {
            Some((head, revision)) => (head, Some(revision)),
            None => (rest, None),
        };
        let invalid = |part| move |source| DocumentUriError::InvalidPart { part, source };
        Ok(Self {
            document_id: DocumentId::try_new(document).map_err(invalid("document id"))?,
            revision_id: revision
                .map(DocumentRevisionId::try_new)
                .transpose()
                .map_err(invalid("revision id"))?,
            node: node
                .map(DocumentNodeId::try_new)
                .transpose()
                .map_err(invalid("node id"))?,
        })
    }
}

impl TryFrom<String> for DocumentUri {
    type Error = DocumentUriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<DocumentUri> for String {
    fn from(uri: DocumentUri) -> Self {
        uri.to_string()
    }
}

impl schemars::JsonSchema for DocumentUri {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DocumentUri".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^document://[^@#]+(@[^#]+)?(#.+)?$"
        })
    }
}
