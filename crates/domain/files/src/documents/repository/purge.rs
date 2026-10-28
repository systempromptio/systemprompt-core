//! Document store rows that leave with a deleted user.
//!
//! Documents, folders and revisions are shared content: they move to a new
//! owner through `FilesOwnerReassignment` rather than being purged. Only the
//! user's own leases, presence rows and user-subject grants are deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::user_purge_tables;

user_purge_tables!(
    "systemprompt-files",
    [
        ("document_presence", "user_id"),
        ("document_locks", "holder_user_id"),
        ("document_grants", "subject_value", "subject_kind = 'user'"),
    ]
);
