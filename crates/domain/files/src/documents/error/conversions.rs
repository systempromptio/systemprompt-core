//! Conversions from codec, targeting and authorization errors into
//! [`DocumentsError`], lifting the variants callers act on to the top level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::authz::AuthzError;
use systemprompt_models::authz::SubjectSourceError;
use systemprompt_provider_contracts::documents::{CodecError, TargetError};

use super::{AuthzFailure, DocumentsError};

impl From<CodecError> for DocumentsError {
    fn from(error: CodecError) -> Self {
        match error {
            CodecError::StaleAnchor {
                node,
                expected,
                actual,
                current_text,
            } => Self::StaleAnchor {
                node,
                expected,
                actual,
                current_text,
            },
            CodecError::UnknownNode { node } => Self::UnknownNode { node },
            other => Self::Codec(other),
        }
    }
}

impl From<TargetError> for DocumentsError {
    fn from(error: TargetError) -> Self {
        match error {
            TargetError::FindNone { find, closest } => Self::FindNone { find, closest },
            TargetError::UnknownNode { node } => Self::UnknownNode { node },
            TargetError::MissingHash { .. } => Self::Codec(CodecError::Malformed {
                detail: "expected_hash is required when node names a node id",
            }),
        }
    }
}

impl From<SubjectSourceError> for DocumentsError {
    fn from(error: SubjectSourceError) -> Self {
        Self::Authz(AuthzFailure::Subjects(error))
    }
}

impl From<AuthzError> for DocumentsError {
    fn from(error: AuthzError) -> Self {
        Self::Authz(AuthzFailure::Catalog(error))
    }
}
