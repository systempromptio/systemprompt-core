//! [`NotificationsExtension`]: registers the notifications tables and
//! migrations, the `notifications` config plane and the retention job, and
//! depends on the `users` extension.
//!
//! It declares no runner service: the notifications consumer needs the
//! pipeline seams the runtime builder assembles, so the builder starts it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_traits::Job;

use crate::jobs::NotificationRetentionJob;
use crate::plane::NotificationsPlane;
use crate::schema::NOTIFICATION_SCHEMAS;

#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationsExtension;

impl Extension for NotificationsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "notifications",
            name: "Notifications",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        NOTIFICATION_SCHEMAS
            .iter()
            .map(|(table, sql, columns)| {
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect())
            })
            .collect()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![Arc::new(NotificationsPlane)]
    }

    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![Arc::new(NotificationRetentionJob)]
    }
}

register_extension!(NotificationsExtension);

user_purge_tables!(
    "systemprompt-notifications",
    [
        ("notification_subscriptions", "subject_user_id"),
        ("notification_delivery_failures", "recipient_user_id"),
    ]
);
