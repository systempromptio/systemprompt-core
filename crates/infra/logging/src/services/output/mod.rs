//! Process-global log-event publishing.
//!
//! Holds the once-initialised [`LogEventPublisher`] and a startup-mode flag,
//! letting any crate emit a [`LogEventData`] via [`publish_log`] without
//! threading the publisher through call sites. Before a publisher is installed
//! (e.g. early boot), `publish_log` is a no-op.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use chrono::Utc;
use systemprompt_traits::{LogEventData, LogEventLevel, LogEventPublisher};

static STARTUP_MODE: AtomicBool = AtomicBool::new(true);

static LOG_PUBLISHER: OnceLock<Arc<dyn LogEventPublisher>> = OnceLock::new();

pub fn set_startup_mode(enabled: bool) {
    STARTUP_MODE.store(enabled, Ordering::SeqCst);
}

#[must_use]
pub fn is_startup_mode() -> bool {
    STARTUP_MODE.load(Ordering::SeqCst)
}

pub fn set_log_publisher(publisher: Arc<dyn LogEventPublisher>) {
    if LOG_PUBLISHER.set(publisher).is_err() {
        tracing::warn!("Log publisher already initialized, ignoring duplicate registration");
    }
}

#[must_use]
pub fn get_log_publisher() -> Option<&'static Arc<dyn LogEventPublisher>> {
    LOG_PUBLISHER.get()
}

pub fn publish_log(level: LogEventLevel, module: &str, message: &str) {
    if let Some(publisher) = LOG_PUBLISHER.get() {
        publisher.publish_log(LogEventData::new(Utc::now(), level, module, message));
    }
}
