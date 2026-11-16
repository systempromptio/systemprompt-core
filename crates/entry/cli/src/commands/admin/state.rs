//! Kernel-state values the admin commands read through the host.
//!
//! An admin command reads what it needs from the state the kernel resolved:
//! the booted profile, the application context and the configuration. A
//! command that prints its own report returns [`rendered`], an output the
//! kernel does not render again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_runtime::AppContext;

pub(crate) async fn get<T: Clone + Send + Sync + 'static>(host: &dyn CommandHost) -> Result<T> {
    Ok(host.resolved().await?.get::<T>()?)
}

pub(crate) async fn app(host: &dyn CommandHost) -> Result<Arc<AppContext>> {
    get::<Arc<AppContext>>(host).await
}

pub(crate) async fn boot(host: &dyn CommandHost) -> Result<Arc<BootContext>> {
    get::<Arc<BootContext>>(host).await
}

pub(crate) async fn config(host: &dyn CommandHost) -> Result<Arc<Config>> {
    get::<Arc<Config>>(host).await
}

pub(crate) fn rendered() -> CommandOutput {
    CommandOutput::text(String::new()).with_skip_render()
}
