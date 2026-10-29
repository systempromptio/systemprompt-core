//! The notification inbox: realtime frames from the bridge stream, catch-up
//! over the bridge JWT, and the persisted inbox the desktop reads.
//!
//! The comms stream decodes `notification.*` frames into
//! [`NotificationSignal`]s and announces every (re)open; the inbox task
//! ([`run::run_loop`]) folds them into the shared [`InboxHandle`], fetches
//! whatever the stream missed newest-first until it meets a held item, and
//! queues desktop toasts. Nothing here touches a windowing library: the GUI
//! drains the queued [`InboxToast`]s and renders them per platform.
//!
//! Only notifications at [`TOAST_FLOOR`] or above toast. The bridge manifest
//! carries no per-user toast floor, so the floor is fixed at human attention.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catch_up;
mod handle;
mod run;
mod signal;
mod state;
mod toast;

pub use catch_up::{CATCH_UP_PAGE, catch_up};
pub use handle::{InboxHandle, InboxRequest, InboxRequestReceiver};
pub use run::{InboxDeps, run_loop};
pub use signal::{NotificationSignal, NotificationSignalReceiver, NotificationSignals, signal_for};
pub use state::{INBOX_FILE_NAME, InboxState, InboxStateError, MAX_ITEMS, PageMerge, inbox_path};
pub use toast::{CATCH_UP_TOASTS, InboxToast, ItemToast, TOAST_FLOOR, plan_toasts};
