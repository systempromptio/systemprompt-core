//! Kernel-state values the admin commands read through the host.
//!
//! The booted profile, the application context and the configuration come
//! from the kernel host, which never places them in the state a member's
//! command resolves; anything else is read from the resolved kernel state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_runtime::AppContext;

pub(crate) async fn get<T: Clone + Send + Sync + 'static>(host: &dyn KernelHost) -> Result<T> {
    Ok(host.resolved().await?.get::<T>()?)
}

pub(crate) async fn app(host: &dyn KernelHost) -> Result<Arc<AppContext>> {
    Ok(host.app_context().await?)
}

pub(crate) fn boot(host: &dyn KernelHost) -> Result<Arc<BootContext>> {
    Ok(host.boot()?)
}

pub(crate) fn config(host: &dyn KernelHost) -> Result<Arc<Config>> {
    Ok(KernelHost::config(host)?)
}
