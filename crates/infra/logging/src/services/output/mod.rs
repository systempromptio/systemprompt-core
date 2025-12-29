use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use chrono::Utc;
use systemprompt_traits::{LogEventData, LogEventLevel, LogEventPublisher};

static OUTPUT_MODE: AtomicU8 = AtomicU8::new(0);
static STARTUP_MODE: AtomicBool = AtomicBool::new(true); // Default true for CLI startup

static LOG_PUBLISHER: OnceLock<Arc<dyn LogEventPublisher>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputMode {
    #[default]
    Cli = 0,
    Tui = 1,
    Headless = 2,
}

impl From<u8> for OutputMode {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Tui,
            2 => Self::Headless,
            _ => Self::Cli,
        }
    }
}

pub fn set_output_mode(mode: OutputMode) {
    OUTPUT_MODE.store(mode as u8, Ordering::SeqCst);
}

#[must_use]
pub fn get_output_mode() -> OutputMode {
    OutputMode::from(OUTPUT_MODE.load(Ordering::SeqCst))
}

#[must_use]
pub fn is_console_output_enabled() -> bool {
    get_output_mode() == OutputMode::Cli
}

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

pub fn init_tui_mode(publisher: Arc<dyn LogEventPublisher>) {
    set_output_mode(OutputMode::Tui);
    set_log_publisher(publisher);
}

pub fn publish_log(level: LogEventLevel, module: &str, message: &str) {
    if let Some(publisher) = LOG_PUBLISHER.get() {
        publisher.publish_log(LogEventData::new(Utc::now(), level, module, message));
    }
}
