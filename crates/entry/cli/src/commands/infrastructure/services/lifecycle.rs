//! The supervised process sets the `infra services` commands drive.
//!
//! The agent and MCP members contribute one `ServiceProcessSet` each to the
//! sealed `ServiceProcessSlot`; these commands are kernel code, so they read
//! the built [`ServiceProcessSets`] from the application kernel's state and
//! name neither member. A member that is not admitted contributes no set: the
//! commands for its kind refuse with a typed error while the others run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_extension::service_process::ServiceProcessSets;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    DynServiceProcessSet, ServiceProcessKind, ServiceProcessStatus, ServiceRun, StatusDepth,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProcessSetError {
    #[error("no admitted member supervises {0:?} processes")]
    NotAdmitted(ServiceProcessKind),
}

pub(crate) fn process_sets(ctx: &Arc<AppContext>) -> Arc<ServiceProcessSets> {
    ctx.kernel()
        .state()
        .find::<Arc<ServiceProcessSets>>()
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn process_set(
    ctx: &Arc<AppContext>,
    kind: ServiceProcessKind,
) -> Result<DynServiceProcessSet> {
    process_sets(ctx)
        .of_kind(kind)
        .cloned()
        .ok_or_else(|| ProcessSetError::NotAdmitted(kind).into())
}

pub(crate) async fn statuses_of(
    ctx: &Arc<AppContext>,
    kind: ServiceProcessKind,
    depth: StatusDepth,
) -> Result<Vec<ServiceProcessStatus>> {
    match process_sets(ctx).of_kind(kind) {
        Some(set) => Ok(set.statuses(depth).await?),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn single_run(runs: Vec<ServiceRun>, name: &str) -> Result<()> {
    let mut runs = runs.into_iter();
    let Some(run) = runs.next() else {
        anyhow::bail!("{name} is not a supervised service");
    };
    run.outcome?;
    for run in runs {
        run.outcome?;
    }
    Ok(())
}
