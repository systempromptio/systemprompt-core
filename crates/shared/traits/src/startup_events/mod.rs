//! Startup event channel and re-exports.
//!
//! Submodules:
//! - [`events`](self) — the [`StartupEvent`] enum;
//! - [`types`](self) — [`Phase`], [`ServiceInfo`], [`ModuleInfo`];
//! - [`ext`](self) — extension traits ([`StartupEventExt`],
//!   [`OptionalStartupEventExt`]) for emitting events ergonomically.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod events;
mod ext;
mod ext_optional;
mod types;

pub use events::StartupEvent;
pub use ext::StartupEventExt;
pub use ext_optional::OptionalStartupEventExt;
pub use types::{ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType};

use futures::channel::mpsc;

pub type StartupEventSender = mpsc::UnboundedSender<StartupEvent>;

pub type StartupEventReceiver = mpsc::UnboundedReceiver<StartupEvent>;

pub fn startup_channel() -> (StartupEventSender, StartupEventReceiver) {
    mpsc::unbounded()
}
