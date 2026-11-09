//! The second launch's side of the single-instance lock: asking an older
//! running bridge to hand over, or asking the running one to focus its window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::Duration;

use super::{
    RunningSidecar, SingletonGuard, SingletonResult, clear_running_port, handoff,
    read_running_instance, try_acquire_gui,
};
use crate::takeover::{self, SecondLaunch};

pub(crate) fn take_over_older_instance() -> Option<SingletonGuard> {
    match try_acquire_gui() {
        SingletonResult::Acquired(guard) => return Some(guard),
        SingletonResult::Error(_) => return None,
        SingletonResult::AlreadyRunning => {},
    }
    let instance = read_running_instance()?;
    let this = crate::brand::brand().version;
    let running = instance.version.as_deref().unwrap_or("unknown");
    if takeover::second_launch(this, instance.version.as_deref()) == SecondLaunch::Focus {
        tracing::info!(
            this,
            running,
            "second launch: the running bridge is the same or newer; focusing it"
        );
        return None;
    }
    if !control_handshake(&instance, takeover::HANDOFF_PATH) {
        tracing::info!(
            this,
            running,
            "second launch: the running bridge did not accept a handoff; focusing it instead"
        );
        return None;
    }
    tracing::info!(
        this,
        running,
        pid = instance.pid,
        "second launch: the older running bridge is handing over"
    );
    let guard = handoff::wait_for_lock();
    if guard.is_none() {
        tracing::warn!(
            pid = instance.pid,
            "second launch: the older bridge accepted the handoff but still holds the lock; this \
             launch does not start a second bridge, and asks the running one to focus or exits"
        );
    }
    guard
}

pub(crate) fn ping_focus_running_instance() -> bool {
    let Some(instance) = read_running_instance() else {
        return false;
    };
    if control_handshake(&instance, takeover::FOCUS_PATH) {
        return true;
    }
    if let Err(e) = clear_running_port() {
        crate::stdio::diag(&format!("remove stale bridge sidecar: {e}"));
    }
    false
}

fn control_handshake(instance: &RunningSidecar, path: &str) -> bool {
    let Ok(parsed) = format!("127.0.0.1:{}", instance.port).parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&parsed, Duration::from_millis(250)) else {
        return false;
    };
    if stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .is_err()
        || stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .is_err()
    {
        return false;
    }
    let request = takeover::control_request(path, &instance.token);
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled = filled.saturating_add(n),
            Err(_) => return false,
        }
    }
    buf.get(..filled)
        .is_some_and(|got| got.starts_with(b"HTTP/1.1 204"))
}
