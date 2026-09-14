//! Process-wide CLI output mode flags and the pre-init notice buffer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static STARTUP_MODE: AtomicBool = AtomicBool::new(true);

static STRUCTURED_MODE: AtomicBool = AtomicBool::new(false);

static STRUCTURED_EMITTED: AtomicBool = AtomicBool::new(false);

static NOTICE_BUFFER: OnceLock<Mutex<Vec<BufferedNotice>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct BufferedNotice {
    pub level: String,
    pub text: String,
}

pub fn set_startup_mode(enabled: bool) {
    STARTUP_MODE.store(enabled, Ordering::SeqCst);
}

#[must_use]
pub fn is_startup_mode() -> bool {
    STARTUP_MODE.load(Ordering::SeqCst)
}

pub fn set_structured_output(enabled: bool) {
    STRUCTURED_MODE.store(enabled, Ordering::SeqCst);
}

#[must_use]
pub fn is_structured_output() -> bool {
    STRUCTURED_MODE.load(Ordering::SeqCst)
}

pub fn mark_structured_emitted() {
    STRUCTURED_EMITTED.store(true, Ordering::SeqCst);
}

pub fn reset_structured_emitted() {
    STRUCTURED_EMITTED.store(false, Ordering::SeqCst);
}

#[must_use]
pub fn structured_was_emitted() -> bool {
    STRUCTURED_EMITTED.load(Ordering::SeqCst)
}

fn notice_buffer() -> &'static Mutex<Vec<BufferedNotice>> {
    NOTICE_BUFFER.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn buffer_notice(level: &str, text: &str) {
    if let Ok(mut buf) = notice_buffer().lock() {
        buf.push(BufferedNotice {
            level: level.to_owned(),
            text: text.to_owned(),
        });
    }
}

#[must_use]
pub fn drain_notices() -> Vec<BufferedNotice> {
    notice_buffer()
        .lock()
        .map(|mut buf| std::mem::take(&mut *buf))
        .unwrap_or_default()
}
