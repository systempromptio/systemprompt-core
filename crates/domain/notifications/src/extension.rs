//! [`NotificationsExtension`]: registers the notifications tables and
//! migrations, the `notifications` config plane and the retention job, and
//! depends on the `users` extension.
//!
//! It provides the producer sink (`DynNotificationSink`, the outbox sink over
//! the write pool) that extensions emitting notifications require, and two
//! empty slots, the [`NotificationsServiceSlot`] and the
//! [`NotificationsConsumerSlot`]. The service and the consumer need the
//! pipeline seams the host assembles, so the host binds them. The serving
//! host binds both and delivers escalations in process; a child host binds
//! only the service and leaves escalated delivery to the serving host's
//! consumer. The consumer runs as this extension's runner service once its
//! slot is bound, and reports its backlog gauges on every sweep. Its backlog
//! is reported on `/health/detail` through `diagnostics()`, never `/readyz`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_extension::{DiagnosticProbe, MetricDescriptor, MetricKind};
use systemprompt_traits::RunnerService;

use crate::consumer::{BacklogDiagnostic, NotificationsConsumerSlot};
use crate::pipeline::metrics::{BACKLOG_OLDEST_SECONDS, BACKLOG_PENDING};
use crate::plane::NotificationsPlane;
use crate::schema::NOTIFICATION_SCHEMAS;
use crate::service::NotificationsServiceSlot;
use crate::sink::OutboxNotificationSink;

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
        ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        let db = Arc::clone(state.state().get::<DbPool>()?);
        state.provide(OutboxNotificationSink::over_write_pool(
            &db,
            &ctx.instance_id(),
        ))?;
        state.provide(NotificationsServiceSlot::default())?;
        state.provide(NotificationsConsumerSlot::default())
    }

    fn runner_services(&self, ctx: &dyn ExtensionContext) -> Vec<Arc<dyn RunnerService>> {
        ctx.extension_state()
            .find::<NotificationsConsumerSlot>()
            .and_then(|slot| slot.get().cloned())
            .map(|consumer| consumer as Arc<dyn RunnerService>)
            .into_iter()
            .collect()
    }

    fn metrics(&self) -> Vec<MetricDescriptor> {
        vec![
            MetricDescriptor {
                name: BACKLOG_PENDING,
                help: "Notification rows committed and not yet delivered",
                kind: MetricKind::Gauge,
            },
            MetricDescriptor {
                name: BACKLOG_OLDEST_SECONDS,
                help: "Age in seconds of the oldest undelivered notification row",
                kind: MetricKind::Gauge,
            },
        ]
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
