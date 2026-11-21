//! Document store rows that leave with a deleted user.
//!
//! Documents, folders and revisions are shared content: they move to a new
//! owner through the files member's owner reassignments rather than being
//! purged. Only the
//! user's own leases, presence rows, live-session peer ids and user-subject
//! grants are deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{PurgePredicate, UserPurgeTable};

pub(crate) const DOCUMENT_USER_PURGES: &[UserPurgeTable] = &[
    UserPurgeTable {
        table: "document_presence",
        column: "user_id",
        predicate: None,
    },
    UserPurgeTable {
        table: "document_collab_peers",
        column: "user_id",
        predicate: None,
    },
    UserPurgeTable {
        table: "document_locks",
        column: "holder_user_id",
        predicate: None,
    },
    UserPurgeTable {
        table: "document_grants",
        column: "subject_value",
        predicate: Some(PurgePredicate {
            column: "subject_kind",
            any_of: &["user"],
        }),
    },
    UserPurgeTable {
        table: "document_access_requests",
        column: "requester_user_id",
        predicate: None,
    },
];
