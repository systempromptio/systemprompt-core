//! Shared real-subprocess and real-TCP-listener fixtures for the MCP
//! lifecycle integration tests — no mocks of the OS surface.

use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

pub fn spawn_sleep(seconds: u64) -> Child {
    Command::new("sleep")
        .arg(seconds.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .expect("`sleep` must be on PATH in the test environment")
}

pub fn spawn_with_orphan_child(seconds: u64) -> (u32, u32) {
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

    let _ = parent.wait();

    (parent_pid, grandchild_pid)
}

fn read_first_line_pid(mut stdout: std::process::ChildStdout) -> u32 {
    use std::io::Read;
    let mut buf = [0u8; 16];
    let mut total = 0;

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

pub fn bind_ephemeral_port() -> (StdTcpListener, u16) {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("ephemeral bind must succeed");
    let port = listener.local_addr().expect("local_addr").port();
    (listener, port)
}

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
