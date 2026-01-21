//! Startup events for tracking application initialization.

mod events;
mod ext;
mod types;

pub use events::StartupEvent;
pub use ext::{OptionalStartupEventExt, StartupEventExt};
pub use types::{ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType};

use futures::channel::mpsc;

pub type StartupEventSender = mpsc::UnboundedSender<StartupEvent>;

pub type StartupEventReceiver = mpsc::UnboundedReceiver<StartupEvent>;

pub fn startup_channel() -> (StartupEventSender, StartupEventReceiver) {
    mpsc::unbounded()
}
