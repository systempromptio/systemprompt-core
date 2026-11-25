//! [`NotificationsExtension`]: contributes the notifications tables and
//! migrations, the `notifications` config plane, the retention job and the
//! user-purge rows of its tables.
//!
//! It provides the producer sink (`DynNotificationSink`, the outbox sink over
//! the write pool, stamped with the kernel's instance id) that members
//! emitting notifications require, and, after the slot is built, the
//! `DynNotificationInbox` other capabilities' surfaces work a user's feed
//! through. The service and the consumer are
//! assembled over the built
//! [`NotificationSeams`](crate::contract::NotificationSeams) when the kernel
//! builds the [`NotificationsSlot`], which creates their cells, binds them and
//! returns the [`NotificationsServiceSlot`] read half; the build reads the
//! kernel's `Arc<EventRouter>`, `Arc<NotificationsProfile>` and the host's
//! `EscalationDelivery`. No member provides or binds either cell. The
//! consumer runs as this member's runner service, and reports its backlog
//! gauges on every sweep and its backlog on `/health/detail`, never
//! `/readyz`.
//!
//! It opens the [`NotificationsSlot`] itself, so the built value is present
//! whenever this member is admitted and absent when the profile disables it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_extension::{
    DiagnosticProbe, DynNotificationInbox, DynNotificationSink, MetricDescriptor, MetricKind,
};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::RunnerService;

use crate::consumer::{BacklogDiagnostic, BoundConsumerRunner};
use crate::contract::NotificationsSlot;
use crate::jobs::NotificationRetentionJob;
use crate::pipeline::metrics::{BACKLOG_OLDEST_SECONDS, BACKLOG_PENDING};
use crate::plane::NotificationsPlane;
use crate::schema::NOTIFICATION_SCHEMAS;
use crate::service::{NotificationsServiceSlot, SlotInbox};
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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for (table, sql, columns) in NOTIFICATION_SCHEMAS {
            c.schema(
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect()),
            );
        }
        c.cross_table("users")
            .migrations(extension_migrations!())
            .config_plane(Arc::new(NotificationsPlane))
            .provide(
                |r: &Resolved<'_>| -> Result<DynNotificationSink, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    let instance = r.get::<InstanceId>()?;
                    Ok(OutboxNotificationSink::over_write_pool(&db, &instance))
                },
            )
            .provide_after_governance(
                |r: &Resolved<'_>| -> Result<DynNotificationInbox, ProvideError> {
                    let slot = r.get::<Arc<NotificationsServiceSlot>>()?;
                    Ok(Arc::new(SlotInbox(slot)))
                },
            )
            .runner_service(
                |r: &Resolved<'_>| -> Result<Arc<dyn RunnerService>, FactoryError> {
                    let slot = r.get::<Arc<NotificationsServiceSlot>>()?;
                    Ok(BoundConsumerRunner::new(slot.consumer().clone()).into_service())
                },
            )
            .diagnostic(
                |r: &Resolved<'_>| -> Result<Arc<dyn DiagnosticProbe>, FactoryError> {
                    let slot = r.get::<Arc<NotificationsServiceSlot>>()?;
                    Ok(BacklogDiagnostic::new(slot.consumer().clone()).into_probe())
                },
            )
            .metric(MetricDescriptor {
                name: BACKLOG_PENDING,
                help: "Notification rows committed and not yet delivered",
                kind: MetricKind::Gauge,
            })
            .metric(MetricDescriptor {
                name: BACKLOG_OLDEST_SECONDS,
                help: "Age in seconds of the oldest undelivered notification row",
                kind: MetricKind::Gauge,
            })
            .job(NotificationRetentionJob)
            .user_purge(UserPurgeTable {
                table: "notification_subscriptions",
                column: "subject_user_id",
                predicate: None,
            })
            .user_purge(UserPurgeTable {
                table: "notification_delivery_failures",
                column: "recipient_user_id",
                predicate: None,
            });
        c.slot::<NotificationsSlot>();
        Ok(())
    }
}
