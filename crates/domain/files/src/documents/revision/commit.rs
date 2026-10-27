//! [`CommitPlan`]: how a commit lands, decided from the head, the base the
//! caller edited and the document format.
//!
//! - The base is the head: bytes are stored as given, ops are applied to the
//!   head's bytes.
//! - The base is stale (or absent while a head exists): Office formats
//!   conflict, because their archives cannot be merged line by line; text
//!   formats (html, markdown, text) merge three ways, with ops first applied to
//!   the base's bytes. A stale commit without a base has nothing to merge from
//!   and conflicts.
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
    Apply,
    MergeBytes,
    MergeOps,
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
        if base == Some(head) {
            return match body {
                CommitBody::Bytes(_) => Self::Store,
                CommitBody::Ops(_) => Self::Apply,
            };
        }
        if base.is_none() || !format.supports_three_way_merge() {
            return Self::Conflict;
        }
        match body {
            CommitBody::Bytes(_) => Self::MergeBytes,
            CommitBody::Ops(_) => Self::MergeOps,
        }
    }

    #[must_use]
    pub const fn merges(self) -> bool {
        matches!(self, Self::MergeBytes | Self::MergeOps)
    }
}
