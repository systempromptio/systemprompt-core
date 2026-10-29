//! The notifications tables: each table's declarative DDL and the columns the
//! schema validator requires of it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub type SchemaEntry = (&'static str, &'static str, &'static [&'static str]);

pub const NOTIFICATION_SCHEMAS: &[SchemaEntry] = &[
    (
        "notification_streams",
        include_str!("../schema/notification_streams.sql"),
        &[
            "id",
            "audience",
            "publishers",
            "attention_default",
            "source",
        ],
    ),
    (
        "notification_subscriptions",
        include_str!("../schema/notification_subscriptions.sql"),
        &[
            "id",
            "subject",
            "target_stream",
            "target_kind",
            "target_id",
            "source",
        ],
    ),
    (
        "notification_rules",
        include_str!("../schema/notification_rules.sql"),
        &[
            "id",
            "priority",
            "owner_user_id",
            "match",
            "action",
            "source",
        ],
    ),
    (
        "notifications",
        include_str!("../schema/notifications.sql"),
        &[
            "id",
            "recipient_user_id",
            "kind",
            "attention",
            "coalesce_key",
            "read_at",
            "outbox_id",
        ],
    ),
    (
        "notification_facts",
        include_str!("../schema/notification_facts.sql"),
        &["notification_id", "outbox_id", "recipient_user_id"],
    ),
    (
        "notification_receipts",
        include_str!("../schema/notification_receipts.sql"),
        &["id", "notification_id", "surface", "state"],
    ),
    (
        "notification_delivery_failures",
        include_str!("../schema/notification_delivery_failures.sql"),
        &["id", "outbox_id", "stage", "reason"],
    ),
];
