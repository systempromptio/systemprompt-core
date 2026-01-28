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
    let _ = LOG_PUBLISHER.set(publisher);
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
