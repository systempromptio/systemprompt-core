//! Picking a port that the production reclaim path will also consider free.
//!
//! Binding `127.0.0.1` is not that test. Rust sets `SO_REUSEADDR`, so a
//! loopback bind succeeds even when another process already listens on the
//! wildcard address for the same port — macOS's AirPlay Receiver holds 5000
//! exactly that way. The code under test finds that holder through
//! `lsof -ti :<port>` and refuses the port, so a probe that disagrees hands the
//! test a port that is provably in use.

use std::net::TcpListener;

#[must_use]
pub fn port_is_unheld(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

#[must_use]
pub fn bind_in_range(range: std::ops::Range<u16>) -> Option<TcpListener> {
    range.clone().find_map(|port| {
        if !port_is_unheld(port) {
            return None;
        }
        TcpListener::bind(("127.0.0.1", port)).ok()
    })
}

#[must_use]
pub fn free_port_in_range(range: std::ops::Range<u16>) -> Option<u16> {
    let listener = bind_in_range(range)?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    Some(port)
}
