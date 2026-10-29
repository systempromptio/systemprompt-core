//! The document store tables: each table's declarative schema and the
//! columns the schema validator requires of it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) type SchemaEntry = (&'static str, &'static str, &'static [&'static str]);

pub(crate) const DOCUMENT_SCHEMAS: &[SchemaEntry] = &[
    (
        "document_folders",
        include_str!("../schema/document_folders.sql"),
        &["id", "parent_id", "name", "ancestor_ids", "owner_user_id"],
    ),
    (
        "document_blobs",
        include_str!("../schema/document_blobs.sql"),
        &["content_sha256", "size_bytes", "storage_id"],
    ),
    (
        "documents",
        include_str!("../schema/documents.sql"),
        &[
            "id",
            "folder_id",
            "name",
            "format",
            "head_revision_id",
            "owner_user_id",
        ],
    ),
    (
        "document_revisions",
        include_str!("../schema/document_revisions.sql"),
        &[
            "id",
            "document_id",
            "content_sha256",
            "author_user_id",
            "created_at",
        ],
    ),
    (
        "document_grants",
        include_str!("../schema/document_grants.sql"),
        &[
            "id",
            "document_id",
            "folder_id",
            "subject_kind",
            "subject_value",
            "subject_display",
            "access_level",
        ],
    ),
    (
        "document_locks",
        include_str!("../schema/document_locks.sql"),
        &["document_id", "holder_id", "expires_at"],
    ),
    (
        "document_presence",
        include_str!("../schema/document_presence.sql"),
        &["document_id", "user_id", "source", "expires_at"],
    ),
    (
        "document_edit_sessions",
        include_str!("../schema/document_edit_sessions.sql"),
        &["id", "document_id", "closed_at"],
    ),
    (
        "document_collab_updates",
        include_str!("../schema/document_collab_updates.sql"),
        &["session_id", "seq", "payload"],
    ),
    (
        "document_text_index",
        include_str!("../schema/document_text_index.sql"),
        &["document_id", "revision_id", "body"],
    ),
    (
        "document_node_ids",
        include_str!("../schema/document_node_ids.sql"),
        &["document_id", "revision_id", "node_id", "path", "hash8"],
    ),
    (
        "document_node_counters",
        include_str!("../schema/document_node_counters.sql"),
        &["document_id", "writer", "next"],
    ),
    (
        "document_write_acks",
        include_str!("../schema/document_write_acks.sql"),
        &["document_id", "op_id", "revision_id", "ack"],
    ),
    (
        "document_comments",
        include_str!("../schema/document_comments.sql"),
        &["id", "document_id", "node_id", "author_user_id", "body"],
    ),
    (
        "document_proposals",
        include_str!("../schema/document_proposals.sql"),
        &[
            "id",
            "document_id",
            "base_revision_id",
            "proposed_revision_id",
            "status",
        ],
    ),
    (
        "document_path_changes",
        include_str!("../schema/document_path_changes.sql"),
        &["id", "document_id", "from_path", "to_path", "changed_at"],
    ),
    (
        "document_grant_events",
        include_str!("../schema/document_grant_events.sql"),
        &["id", "event", "target_kind", "target_id", "created_at"],
    ),
    (
        "document_agent_reads",
        include_str!("../schema/document_agent_reads.sql"),
        &["id", "document_id", "user_id", "agent_name", "created_at"],
    ),
    (
        "document_access_requests",
        include_str!("../schema/document_access_requests.sql"),
        &["id", "document_id", "requester_user_id", "status"],
    ),
    (
        "document_uploads",
        include_str!("../schema/document_uploads.sql"),
        &[
            "id",
            "owner_user_id",
            "declared_sha256",
            "status",
            "expires_at",
        ],
    ),
    (
        "document_upload_parts",
        include_str!("../schema/document_upload_parts.sql"),
        &["upload_id", "part_number", "size_bytes", "storage_id"],
    ),
];
