//! Liveness maintenance on the `services` registry: heartbeats, the stale-row
//! reaper for this instance, and the cross-instance reap of dead replicas.
//!
//! [`ServiceRepository::reap_stale`] is the only stale-row reaper. It deletes
//! this instance's `error` rows and `running` rows with no pid outright, then
//! asks the injected [`ProcessLiveness`] about every remaining `running` pid
//! and deletes the rows whose process is gone or is not a child this
//! installation spawned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::{ServiceModule, ServiceStatus};
use systemprompt_traits::ProcessLiveness;

use super::claim::INSTANCE_CLAIM_CLASS;
use super::repo::ServiceRepository;
use crate::error::{DatabaseResult, RepositoryError};

/// One `running` row whose recorded pid turned out not to be ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapedService {
    pub name: ServiceName,
    pub module_name: ServiceModule,
    pub pid: u32,
}

/// What one [`ServiceRepository::reap_stale`] pass removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reaped {
    pub stale_rows: u64,
    pub dead: Vec<ReapedService>,
}

impl Reaped {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.stale_rows + self.dead.len() as u64
    }
}

impl ServiceRepository {
    pub async fn reap_stale(&self, liveness: &dyn ProcessLiveness) -> DatabaseResult<Reaped> {
        let stale_rows = sqlx::query!(
            r#"
            DELETE FROM services
            WHERE instance_id = $1
              AND (status = $2 OR (status = $3 AND pid IS NULL))
            "#,
            self.instance_id.as_str(),
            ServiceStatus::Error.as_str(),
            ServiceStatus::Running.as_str()
        )
        .execute(&*self.write_pool)
        .await?
        .rows_affected();

        let candidates = sqlx::query!(
            r#"
            SELECT name, module_name, pid AS "pid!"
            FROM services
            WHERE instance_id = $1 AND status = $2 AND pid IS NOT NULL
            ORDER BY name
            "#,
            self.instance_id.as_str(),
            ServiceStatus::Running.as_str()
        )
        .fetch_all(&*self.write_pool)
        .await?;

        let mut dead = Vec::new();
        for row in candidates {
            let name = ServiceName::new(row.name);
            let module_name = row.module_name.parse::<ServiceModule>().map_err(|source| {
                RepositoryError::Decode {
                    context: "services.module_name".to_owned(),
                    source: Box::new(source),
                }
            })?;
            let Ok(pid) = u32::try_from(row.pid) else {
                self.delete_service(&name).await?;
                dead.push(ReapedService {
                    name,
                    module_name,
                    pid: 0,
                });
                continue;
            };
            if liveness.owns(pid, &name).await {
                continue;
            }
            self.delete_service(&name).await?;
            dead.push(ReapedService {
                name,
                module_name,
                pid,
            });
        }

        Ok(Reaped { stale_rows, dead })
    }

    pub async fn touch_heartbeat(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"UPDATE services SET heartbeat_at = CURRENT_TIMESTAMP WHERE instance_id = $1"#,
            self.instance_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_dead_instances(&self, older_than_secs: i64) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM services
            WHERE heartbeat_at < CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
              AND NOT EXISTS (
                  SELECT 1 FROM pg_locks l
                  WHERE l.locktype = 'advisory'
                    AND l.granted
                    AND l.classid = ($2::int)::oid
                    AND l.objid = (hashtext(services.instance_id))::oid
                    AND l.objsubid = 2
              )
            "#,
            older_than_secs as f64,
            INSTANCE_CLAIM_CLASS
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
