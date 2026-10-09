//! `services` row fixtures.
//!
//! [`seed_running_service`] registers a row, under the context's own instance
//! id, that stands for a service this test process is really hosting — a
//! wiremock backend, a bound listener, a spawned child.
//!
//! Reach for it instead of writing the row directly.
//! `ServiceRepository::cleanup_stale_entries` deletes every
//! `status = 'running' AND pid IS NULL` row across the whole table. That is
//! correct in production, where one instance owns the database, but every test
//! shares one database, so a pid-less registration is deleted by whichever
//! concurrent test next boots an orchestrator. Recording the pid of the process
//! that hosts the service keeps the sweep from claiming the row, and is honest
//! about who owns it.
//!
//! The row is written in one statement. Creating it and then setting the pid
//! leaves a window in which it is `running` with a null pid, and a sweep
//! landing inside that window deletes it — the following `UPDATE` then matches
//! nothing and reports success, so the fixture returns `Ok` having registered
//! nothing.
//!
//! A test that deliberately wants a *stale* row — pid-less, or a dead pid, to
//! drive the sweep itself — should not use this helper; write the row directly
//! so the intent is visible at the call site.

use anyhow::Result;
use systemprompt_runtime::AppContext;

pub async fn seed_running_service(
    ctx: &AppContext,
    name: &str,
    module_name: &str,
    port: u16,
) -> Result<()> {
    let p = ctx.db_pool().pool();
    let pid = i32::try_from(std::process::id()).map_err(|e| anyhow::anyhow!("pid: {e}"))?;
    let port = i32::from(port);
    sqlx::query!(
        "INSERT INTO services (instance_id, name, module_name, status, port, pid)
         VALUES ($5, $1, $2, 'running', $3, $4)
         ON CONFLICT (instance_id, name) DO UPDATE
           SET module_name = $2, status = 'running', port = $3, pid = $4,
               updated_at = CURRENT_TIMESTAMP",
        name,
        module_name,
        port,
        pid,
        ctx.config().instance_id.as_str(),
    )
    .execute(p.as_ref())
    .await
    .map_err(|e| anyhow::anyhow!("seed running service {name}: {e}"))?;
    Ok(())
}
