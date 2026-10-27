//! [`CommitPlan`]: how a commit lands, decided from the head, the base the
//! caller read and the body.
//!
//! - Ops always apply to the head ([`CommitPlan::ApplyOnHead`]), whatever their
//!   base: each anchored op names a node by stable id and quotes that node's
//!   hash, so a commit is refused only when a node it touches changed at the
//!   head. The base only says which revision the ids and hashes were read from.
//! - Bytes whose base is the head are stored as given. Bytes on a stale base
//!   merge three ways for text formats (html, markdown, text); for Office
//!   formats they are diffed against their base into ops that then apply to the
//!   head ([`CommitPlan::DiffOnHead`]). Bytes without a base on a document that
//!   has a head have nothing to merge from and conflict.
//! - The document has no head yet: bytes become the first revision; ops have
//!   nothing to anchor to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::documents::{DocumentFormat, EditOp};

#[derive(Debug, Clone, PartialEq)]
pub enum CommitBody {
    Bytes(Vec<u8>),
    Ops(Vec<EditOp>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitPlan {
    Store,
    ApplyOnHead,
    MergeBytes,
    DiffOnHead,
    Conflict,
    NoHead,
}

impl CommitPlan {
    #[must_use]
    pub fn decide(
        head: Option<&DocumentRevisionId>,
        base: Option<&DocumentRevisionId>,
        format: DocumentFormat,
        body: &CommitBody,
    ) -> Self {
        let Some(head) = head else {
            return match body {
                CommitBody::Bytes(_) => Self::Store,
                CommitBody::Ops(_) => Self::NoHead,
            };
        };
        if matches!(body, CommitBody::Ops(_)) {
            return Self::ApplyOnHead;
        }
        match base {
            Some(base) if base == head => Self::Store,
            None => Self::Conflict,
            Some(_) if format.supports_three_way_merge() => Self::MergeBytes,
            Some(_) => Self::DiffOnHead,
        }
    }

    #[must_use]
    pub fn merges(
        self,
        head: Option<&DocumentRevisionId>,
        base: Option<&DocumentRevisionId>,
    ) -> bool {
        match self {
            Self::MergeBytes | Self::DiffOnHead => true,
            Self::ApplyOnHead => base.is_some_and(|base| Some(base) != head),
            Self::Store | Self::Conflict | Self::NoHead => false,
        }
    }
}
