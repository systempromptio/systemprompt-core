//! Shared fixtures for MCP lifecycle integration tests.
//!
//! Real subprocess (`sleep`) and real TCP listeners — no mocks of the OS
//! surface.

use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

/// Spawns a real `sleep` child that lives `seconds` seconds, returning the
/// [`Child`] handle so the test owns its lifetime. The OS-level PID is what
/// the MCP process layer operates on, so a `sleep` is a sufficient stand-in
/// for any long-running MCP server binary.
pub fn spawn_sleep(seconds: u64) -> Child {
    Command::new("sleep")
        .arg(seconds.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .expect("`sleep` must be on PATH in the test environment")
}

/// Spawns a shell that immediately execs a backgrounded `sleep`, then
/// exits. The grandchild outlives the direct child — modelling an MCP
/// server that forks a helper and dies. Returns
/// `(reaped_parent_pid, grandchild_pid)`. The shell is `wait()`ed
/// internally so the caller never sees a zombie on its books.
pub fn spawn_with_orphan_child(seconds: u64) -> (u32, u32) {
    // `setsid` detaches the grandchild from the parent's process group so
    // it survives the parent exit and is reparented to PID 1. The shell
    // prints the grandchild PID then exits immediately — it does NOT
    // `wait` on the backgrounded child (the child is in a new session,
    // so wait would block forever).
    let mut parent = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "setsid sleep {seconds} >/dev/null 2>&1 < /dev/null & echo $!"
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .expect("`sh` must be on PATH");

    let parent_pid = parent.id();
    let stdout = parent.stdout.take().expect("parent stdout must be piped");
    let grandchild_pid = read_first_line_pid(stdout);

    // Reap the shell — otherwise it becomes a zombie under the test
    // process and `kill(parent_pid, 0)` still returns Ok, masking the
    // "parent exited" check the caller relies on.
    let _ = parent.wait();

    (parent_pid, grandchild_pid)
}

fn read_first_line_pid(mut stdout: std::process::ChildStdout) -> u32 {
    use std::io::Read;
    let mut buf = [0u8; 16];
    let mut total = 0;

    // Bounded wait — the shell echoes the PID before `wait`, so the read
    // returns quickly. If it ever blocks, the test fails fast.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && total < buf.len() {
        match stdout.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if buf[..total].contains(&b'\n') {
                    break;
                }
            },
            Err(_) => break,
        }
    }

    std::str::from_utf8(&buf[..total])
        .unwrap_or("")
        .trim()
        .parse::<u32>()
        .expect("shell must print the grandchild PID")
}

/// Binds a real TCP listener on `127.0.0.1:0` and returns the listener plus
/// the kernel-assigned port. Caller drops the listener to free the port.
pub fn bind_ephemeral_port() -> (StdTcpListener, u16) {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("ephemeral bind must succeed");
    let port = listener.local_addr().expect("local_addr").port();
    (listener, port)
}

/// Spawns an in-process Tokio TCP accept loop on `127.0.0.1:0`, returning
/// `(addr, JoinHandle)`. The loop accepts and immediately closes each
/// connection — sufficient for "is the port live" probes that drive the FD
/// stress test. Drop the handle to stop the listener.
pub async fn spawn_tcp_accept_loop() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("async bind must succeed");
    let addr = listener.local_addr().expect("local_addr");

    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
    });

    (addr, handle)
}
