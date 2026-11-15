//! [`NotificationsExtension`]: contributes the notifications tables and
//! migrations, the `notifications` config plane, the retention job and the
//! user-purge rows of its tables.
//!
//! It provides the producer sink (`DynNotificationSink`, the outbox sink over
//! the write pool, stamped with the kernel's instance id) that members
//! emitting notifications require, and two empty late-bound handles, the
//! [`NotificationsServiceSlot`] and the [`NotificationsConsumerSlot`]. The
//! service and the consumer are assembled over the built
//! [`NotificationSeams`](crate::contract::NotificationSeams) by the
//! composition root, which binds them. The consumer runs as this member's
//! runner service once its handle is bound, and reports its backlog gauges on
//! every sweep and its backlog on `/health/detail`, never `/readyz`.
//!
//! It opens the [`NotificationsSlot`] itself, so the built seams are present
//! whenever this member is admitted and absent when the profile disables it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_extension::{DiagnosticProbe, DynNotificationSink, MetricDescriptor, MetricKind};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::RunnerService;

use crate::consumer::{BacklogDiagnostic, BoundConsumerRunner, NotificationsConsumerSlot};
use crate::contract::NotificationsSlot;
use crate::jobs::NotificationRetentionJob;
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
            .provide(
                |_: &Resolved<'_>| -> Result<NotificationsServiceSlot, ProvideError> {
                    Ok(NotificationsServiceSlot::default())
                },
            )
            .provide(
                |_: &Resolved<'_>| -> Result<NotificationsConsumerSlot, ProvideError> {
                    Ok(NotificationsConsumerSlot::default())
                },
            )
            .runner_service(
                |r: &Resolved<'_>| -> Result<Arc<dyn RunnerService>, FactoryError> {
                    let slot = r.get::<NotificationsConsumerSlot>()?;
                    Ok(BoundConsumerRunner::new(slot).into_service())
                },
            )
            .diagnostic(
                |r: &Resolved<'_>| -> Result<Arc<dyn DiagnosticProbe>, FactoryError> {
                    let slot = r.get::<NotificationsConsumerSlot>()?;
                    Ok(BacklogDiagnostic::new(slot).into_probe())
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
