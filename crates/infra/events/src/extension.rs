//! Extension-framework registration for the events crate.
//!
//! [`EventsExtension`] declares the schema for the `event_outbox` table —
//! the durable relay channel that lets an event published on one replica
//! reach SSE subscribers on every other replica via Postgres LISTEN/NOTIFY.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct EventsExtension;

impl Extension for EventsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "events",
            name: "Events",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new("event_outbox", include_str!("../schema/event_outbox.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "channel".into(),
                    "user_id".into(),
                    "payload".into(),
                    "actor_kind".into(),
                    "actor_id".into(),
                    "created_at".into(),
                ]),
        ]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(EventsExtension);
