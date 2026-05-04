//! Startup event channel and re-exports.
//!
//! Submodules:
//! - [`events`](self) — the [`StartupEvent`] enum;
//! - [`types`](self) — [`Phase`], [`ServiceInfo`], [`ModuleInfo`];
//! - [`ext`](self) — extension traits ([`StartupEventExt`],
//!   [`OptionalStartupEventExt`]) for emitting events ergonomically.

mod events;
mod ext;
mod types;

pub use events::StartupEvent;
pub use ext::{OptionalStartupEventExt, StartupEventExt};
pub use types::{ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType};

use futures::channel::mpsc;

/// Sender side of the startup event channel.
pub type StartupEventSender = mpsc::UnboundedSender<StartupEvent>;

/// Receiver side of the startup event channel.
pub type StartupEventReceiver = mpsc::UnboundedReceiver<StartupEvent>;

/// Construct a fresh unbounded startup event channel.
pub fn startup_channel() -> (StartupEventSender, StartupEventReceiver) {
    mpsc::unbounded()
}
