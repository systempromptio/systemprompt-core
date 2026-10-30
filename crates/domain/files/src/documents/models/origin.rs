//! [`RevisionOrigin`]: where a revision or proposal came from, persisted as
//! the `origin_kind` and `origin_id` columns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{DocumentEditSessionId, DriveDeviceId, McpExecutionId, SessionId};
use systemprompt_models::documents::{RevisionOriginKind, UnknownDocumentValue};

/// Why a stored `origin_kind` / `origin_id` pair is not a revision origin.
#[derive(Debug, thiserror::Error)]
pub enum RevisionOriginError {
    #[error(transparent)]
    Kind(#[from] UnknownDocumentValue),
    #[error("a {kind} origin needs an origin id")]
    MissingId { kind: RevisionOriginKind },
    #[error("a {kind} origin carries no origin id")]
    UnexpectedId { kind: RevisionOriginKind },
    #[error("origin id is not valid for a {kind} origin")]
    Id {
        kind: RevisionOriginKind,
        #[source]
        source: IdValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionOrigin {
    McpExecution(McpExecutionId),
    BridgeDevice(DriveDeviceId),
    WebSession(SessionId),
    EditSession(DocumentEditSessionId),
    Api,
}

impl RevisionOrigin {
    #[must_use]
    pub const fn kind(&self) -> RevisionOriginKind {
        match self {
            Self::McpExecution(_) => RevisionOriginKind::McpExecution,
            Self::BridgeDevice(_) => RevisionOriginKind::BridgeDevice,
            Self::WebSession(_) => RevisionOriginKind::WebSession,
            Self::EditSession(_) => RevisionOriginKind::EditSession,
            Self::Api => RevisionOriginKind::Api,
        }
    }

    #[must_use]
    pub fn id_str(&self) -> Option<&str> {
        match self {
            Self::McpExecution(id) => Some(id.as_str()),
            Self::BridgeDevice(id) => Some(id.as_str()),
            Self::WebSession(id) => Some(id.as_str()),
            Self::EditSession(id) => Some(id.as_str()),
            Self::Api => None,
        }
    }

    #[must_use]
    pub const fn device(&self) -> Option<&DriveDeviceId> {
        match self {
            Self::BridgeDevice(id) => Some(id),
            _ => None,
        }
    }

    pub fn from_columns(kind: &str, id: Option<&str>) -> Result<Self, RevisionOriginError> {
        let kind: RevisionOriginKind = kind.parse()?;
        if kind == RevisionOriginKind::Api {
            return match id {
                None => Ok(Self::Api),
                Some(_) => Err(RevisionOriginError::UnexpectedId { kind }),
            };
        }
        let id = id.ok_or(RevisionOriginError::MissingId { kind })?;
        let invalid = |source| RevisionOriginError::Id { kind, source };
        Ok(match kind {
            RevisionOriginKind::McpExecution => {
                Self::McpExecution(McpExecutionId::try_new(id).map_err(invalid)?)
            },
            RevisionOriginKind::BridgeDevice => {
                Self::BridgeDevice(DriveDeviceId::try_new(id).map_err(invalid)?)
            },
            RevisionOriginKind::WebSession => {
                Self::WebSession(SessionId::try_new(id).map_err(invalid)?)
            },
            RevisionOriginKind::EditSession => {
                Self::EditSession(DocumentEditSessionId::try_new(id).map_err(invalid)?)
            },
            RevisionOriginKind::Api => Self::Api,
        })
    }
}
