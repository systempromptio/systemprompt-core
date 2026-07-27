//! `services` row fixtures.
//!
//! [`seed_running_service`] registers a row that stands for a service this test
//! process is really hosting — a wiremock backend, a bound listener, a spawned
//! child.
//!
//! Reach for it instead of calling `ServiceRepository::create_service` with
//! `status: "running"` directly. `ServiceRepository::cleanup_stale_entries`
//! deletes every `status = 'running' AND pid IS NULL` row across the whole
//! table. That is correct in production, where one instance owns the database,
//! but every test shares one database, so a pid-less registration is deleted by
//! whichever concurrent test next boots an orchestrator. Recording the pid of
//! the process that hosts the service keeps the sweep from claiming the row,
//! and is honest about who owns it.
//!
//! A test that deliberately wants a *stale* row — pid-less, or a dead pid, to
//! drive the sweep itself — should not use this helper; construct the row
//! directly so the intent is visible at the call site.

use anyhow::{Context, Result};
use systemprompt_database::{CreateServiceInput, DbPool, ServiceRepository};

pub async fn seed_running_service(
    pool: &DbPool,
    name: &str,
    module_name: &str,
    port: u16,
) -> Result<()> {
    let repo = ServiceRepository::new(pool).map_err(|e| anyhow::anyhow!("service repo: {e}"))?;
    repo.create_service(CreateServiceInput {
        name,
        module_name,
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("create service row for {name}: {e}"))?;

    let pid = i32::try_from(std::process::id()).context("pid fits in i32")?;
    repo.update_service_pid(name, pid)
        .await
        .map_err(|e| anyhow::anyhow!("record hosting pid for {name}: {e}"))?;
    Ok(())
}
