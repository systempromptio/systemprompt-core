//! Loopback ports a test owns for as long as it needs them.
//!
//! A port probed with `bind(:0)` and released straight away is back in the
//! kernel's pool, so any concurrent bind on the host can take it before the
//! code under test uses it. Every port a test needs comes from here instead,
//! and stays held:
//!
//! - [`ReservedPort`] holds a bound, listening socket. A test whose code under
//!   test accepts a listener passes it on ([`ReservedPort::into_listener`],
//!   [`ReservedPort::into_tokio_listener`]); a test whose code under test must
//!   bind the port itself keeps the reservation until the moment it hands the
//!   port over ([`ReservedPort::release_for_child`]), which is the only window
//!   in which another binder can take it.
//! - A test that needs a port nothing answers on releases a reservation too: a
//!   bound socket that is not listening refuses a connection on Linux but has
//!   its SYN silently dropped on macOS, so only a released port refuses on
//!   both.
//!
//! Binding `127.0.0.1` alone does not prove a port free for the production
//! reclaim path. Rust sets `SO_REUSEADDR`, so a loopback bind succeeds even
//! when another process already listens on the wildcard address for the same
//! port (macOS's AirPlay Receiver holds 5000 exactly that way), and the code
//! under test finds that holder through `lsof -ti :<port>` and refuses the
//! port. A ranged reservation therefore skips any port the wildcard probe finds
//! held.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::ops::Range;

pub struct ReservedPort {
    listener: TcpListener,
    port: u16,
}

impl ReservedPort {
    #[must_use]
    pub fn loopback() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap_or_else(|e| panic!("bind an ephemeral loopback port: {e}"));
        Self::from_listener(listener)
    }

    #[must_use]
    pub fn in_range(range: Range<u16>) -> Option<Self> {
        range.into_iter().find_map(|port| {
            if TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).is_err() {
                return None;
            }
            TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                .ok()
                .map(Self::from_listener)
        })
    }

    #[must_use]
    pub fn below_ephemeral_range() -> Self {
        let range = std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range")
            .unwrap_or_else(|e| panic!("read the ephemeral port range: {e}"));
        let low: u16 = range
            .split_whitespace()
            .next()
            .and_then(|bound| bound.parse().ok())
            .unwrap_or_else(|| panic!("the ephemeral range {range:?} has no numeric low bound"));
        (1024..low)
            .rev()
            .find_map(|port| Self::in_range(port..port + 1))
            .unwrap_or_else(|| panic!("no bindable port below the ephemeral range ({low})"))
    }

    fn from_listener(listener: TcpListener) -> Self {
        let port = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("read a reserved listener's address: {e}"))
            .port();
        Self { listener, port }
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.port))
    }

    #[must_use]
    pub fn listener(&self) -> &TcpListener {
        &self.listener
    }

    #[must_use]
    pub fn into_listener(self) -> TcpListener {
        self.listener
    }

    #[must_use]
    pub fn into_tokio_listener(self) -> tokio::net::TcpListener {
        self.listener
            .set_nonblocking(true)
            .unwrap_or_else(|e| panic!("make the reserved listener non-blocking: {e}"));
        tokio::net::TcpListener::from_std(self.listener)
            .unwrap_or_else(|e| panic!("register the reserved listener with tokio: {e}"))
    }

    #[must_use]
    pub fn release_for_child(self) -> u16 {
        drop(self.listener);
        self.port
    }
}
