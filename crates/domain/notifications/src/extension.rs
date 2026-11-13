//! [`NotificationsExtension`]: registers the notifications tables and
//! migrations, the `notifications` config plane and the retention job, and
//! depends on the `users` extension.
//!
//! It declares no runner service: the notifications consumer needs the
//! pipeline seams the runtime builder assembles, so the builder starts it and
//! binds it into the consumer slot this extension provides. Its backlog is
//! reported on `/health/detail` through `diagnostics()`, never `/readyz`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::DiagnosticProbe;
use systemprompt_extension::prelude::*;

use crate::consumer::{BacklogDiagnostic, NotificationsConsumerSlot};
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

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        state.provide(NotificationsConsumerSlot::default())
    }

    fn diagnostics(&self, ctx: &dyn ExtensionContext) -> Vec<Arc<dyn DiagnosticProbe>> {
        ctx.extension_state()
            .find::<NotificationsConsumerSlot>()
            .map(|slot| BacklogDiagnostic::new(slot.clone()).into_probe())
            .into_iter()
            .collect()
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
