//! Loopback listener for the browser-driven OAuth and checkout callback
//! servers. Binds with `SO_REUSEADDR` so an immediately re-run flow does not
//! fail with `AddrInUse` while the previous run's connections sit in
//! `TIME_WAIT` on the fixed callback port.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpSocket};

pub(crate) fn bind_callback_listener(port: u16) -> std::io::Result<TcpListener> {
    let socket = TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(SocketAddr::from(([127, 0, 0, 1], port)))?;
    socket.listen(1024)
}
