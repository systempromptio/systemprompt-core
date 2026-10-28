//! The refusal codes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

/// The stable code of a refusal; `documents_guide` explains each as
/// `refusal.<code>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    StaleBase,
    GuardMismatch,
    FindNone,
    UnsupportedOp,
    UnsupportedFormat,
    TemplateFailed,
    Forbidden,
    NotFound,
    Trashed,
    Locked,
    FolderNotEmpty,
    AnchorsAffected,
    ProposalConflict,
    ProposalClosed,
    InvalidParams,
    UnknownSubject,
    TooLarge,
    IntegrityMismatch,
    OpIdMismatch,
    SelfReview,
    AccessRequestOpen,
    AccessRequestNotNeeded,
    AccessRequestClosed,
}

impl RefusalCode {
    pub const ALL: [Self; 23] = [
        Self::StaleBase,
        Self::GuardMismatch,
        Self::FindNone,
        Self::UnsupportedOp,
        Self::UnsupportedFormat,
        Self::TemplateFailed,
        Self::Forbidden,
        Self::NotFound,
        Self::Trashed,
        Self::Locked,
        Self::FolderNotEmpty,
        Self::AnchorsAffected,
        Self::ProposalConflict,
        Self::ProposalClosed,
        Self::InvalidParams,
        Self::UnknownSubject,
        Self::TooLarge,
        Self::IntegrityMismatch,
        Self::OpIdMismatch,
        Self::SelfReview,
        Self::AccessRequestOpen,
        Self::AccessRequestNotNeeded,
        Self::AccessRequestClosed,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleBase => "stale_base",
            Self::GuardMismatch => "guard_mismatch",
            Self::FindNone => "find_none",
            Self::UnsupportedOp => "unsupported_op",
            Self::UnsupportedFormat => "unsupported_format",
            Self::TemplateFailed => "template_failed",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Trashed => "trashed",
            Self::Locked => "locked",
            Self::FolderNotEmpty => "folder_not_empty",
            Self::AnchorsAffected => "anchors_affected",
            Self::ProposalConflict => "proposal_conflict",
            Self::ProposalClosed => "proposal_closed",
            Self::InvalidParams => "invalid_params",
            Self::UnknownSubject => "unknown_subject",
            Self::TooLarge => "too_large",
            Self::IntegrityMismatch => "integrity_mismatch",
            Self::OpIdMismatch => "op_id_mismatch",
            Self::SelfReview => "self_review",
            Self::AccessRequestOpen => "access_request_open",
            Self::AccessRequestNotNeeded => "access_request_not_needed",
            Self::AccessRequestClosed => "access_request_closed",
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == code)
    }
}
