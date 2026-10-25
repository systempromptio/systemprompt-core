//! CLI output mode flags and the pre-init notice buffer, owned by the
//! installed subscriber.
//!
//! [`OutputState`] is created once at the CLI composition root, handed to the
//! subscriber installer, and recovered by display sinks through
//! [`OutputState::current`], which downcasts the active dispatcher the same
//! way the database layer is found. With no subscriber carrying one, `current`
//! returns a detached default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use systemprompt_manifest::profile::LogFormat;
use tracing::Subscriber;
use tracing_subscriber::Layer;

#[derive(Debug, Clone)]
pub struct BufferedNotice {
    pub level: String,
    pub text: String,
}

#[derive(Debug)]
struct Inner {
    startup: AtomicBool,
    structured: AtomicBool,
    structured_emitted: AtomicBool,
    json_console: AtomicBool,
    console_armed: AtomicBool,
    notices: Mutex<Vec<BufferedNotice>>,
}

/// Output mode and notice buffer for one process; clones share the same state.
#[derive(Debug, Clone)]
pub struct OutputState {
    inner: Arc<Inner>,
}

impl Default for OutputState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                startup: AtomicBool::new(true),
                structured: AtomicBool::new(false),
                structured_emitted: AtomicBool::new(false),
                json_console: AtomicBool::new(false),
                console_armed: AtomicBool::new(true),
                notices: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl<S: Subscriber> Layer<S> for OutputState {}

impl OutputState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn current() -> Self {
        tracing::dispatcher::get_default(|dispatch| dispatch.downcast_ref::<Self>().cloned())
            .unwrap_or_default()
    }

    pub fn set_startup_mode(&self, enabled: bool) {
        self.inner.startup.store(enabled, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_startup_mode(&self) -> bool {
        self.inner.startup.load(Ordering::SeqCst)
    }

    pub fn set_structured_output(&self, enabled: bool) {
        self.inner.structured.store(enabled, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_structured_output(&self) -> bool {
        self.inner.structured.load(Ordering::SeqCst)
    }

    pub fn mark_structured_emitted(&self) {
        self.inner.structured_emitted.store(true, Ordering::SeqCst);
    }

    pub fn reset_structured_emitted(&self) {
        self.inner.structured_emitted.store(false, Ordering::SeqCst);
    }

    #[must_use]
    pub fn structured_was_emitted(&self) -> bool {
        self.inner.structured_emitted.load(Ordering::SeqCst)
    }

    pub fn set_console_format(&self, format: LogFormat) {
        self.inner
            .json_console
            .store(matches!(format, LogFormat::Json), Ordering::Relaxed);
    }

    #[must_use]
    pub fn console_format(&self) -> LogFormat {
        if self.is_json_console() {
            LogFormat::Json
        } else {
            LogFormat::Text
        }
    }

    pub(crate) fn is_json_console(&self) -> bool {
        self.inner.json_console.load(Ordering::Relaxed)
    }

    pub(crate) fn set_console_armed(&self, armed: bool) {
        self.inner.console_armed.store(armed, Ordering::SeqCst);
    }

    pub(crate) fn is_console_armed(&self) -> bool {
        self.inner.console_armed.load(Ordering::SeqCst)
    }

    pub fn buffer_notice(&self, level: &str, text: &str) {
        if let Ok(mut buf) = self.inner.notices.lock() {
            buf.push(BufferedNotice {
                level: level.to_owned(),
                text: text.to_owned(),
            });
        }
    }

    #[must_use]
    pub fn drain_notices(&self) -> Vec<BufferedNotice> {
        self.inner
            .notices
            .lock()
            .map(|mut buf| std::mem::take(&mut *buf))
            .unwrap_or_default()
    }
}
