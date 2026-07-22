//! Unit tests for the process-global log-event publisher hook.
//!
//! `LOG_PUBLISHER` is a `OnceLock`: the whole sequence (publish before install,
//! first install wins, duplicate install is ignored) must run in one test.

use std::sync::{Arc, Mutex};
use systemprompt_logging::{publish_log, set_log_publisher};
use systemprompt_traits::{LogEventData, LogEventLevel, LogEventPublisher};

#[derive(Default)]
struct CapturingPublisher {
    events: Mutex<Vec<LogEventData>>,
}

impl LogEventPublisher for CapturingPublisher {
    fn publish_log(&self, event: LogEventData) {
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event);
    }
}

impl CapturingPublisher {
    fn messages(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|e| e.message.clone())
            .collect()
    }
}

#[test]
fn publisher_lifecycle_first_install_wins() {
    publish_log(LogEventLevel::Info, "pubtest", "before-install");

    let first = Arc::new(CapturingPublisher::default());
    set_log_publisher(first.clone());
    publish_log(LogEventLevel::Warn, "pubtest", "after-install");

    let second = Arc::new(CapturingPublisher::default());
    set_log_publisher(second.clone());
    publish_log(LogEventLevel::Error, "pubtest", "after-duplicate-install");

    let first_messages = first.messages();
    assert!(
        !first_messages.contains(&"before-install".to_owned()),
        "events published before install must be dropped"
    );
    assert!(first_messages.contains(&"after-install".to_owned()));
    assert!(
        first_messages.contains(&"after-duplicate-install".to_owned()),
        "the first-installed publisher must keep receiving events"
    );
    assert!(
        second.messages().is_empty(),
        "a duplicate install must be ignored"
    );

    let installed = first
        .events
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .map(|e| (e.level, e.module.clone()))
        .collect::<Vec<_>>();
    assert!(installed.iter().all(|(_, module)| module == "pubtest"));
    assert!(installed.iter().any(|(l, _)| *l == LogEventLevel::Warn));
}
