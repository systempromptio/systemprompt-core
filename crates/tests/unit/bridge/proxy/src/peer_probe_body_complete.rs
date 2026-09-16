//! The whoami peer probe reads exactly the declared body and stops: a sibling
//! that keeps the socket open after answering (a keep-alive server ignoring
//! `Connection: close`, a starved runtime closing late) is still identified
//! within the probe budget rather than read as an unidentified listener — the
//! misread that made a second process bind the next port beside a healthy
//! sibling.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use systemprompt_bridge::proxy::identity::{InstallId, WhoAmI};
use systemprompt_bridge::proxy::peer::{PeerIdentity, probe_identity};

fn whoami_body(port: u16, install_id: &InstallId) -> String {
    serde_json::to_string(&WhoAmI::current(port, 1_753_948_800, install_id)).expect("json")
}

// Answers one request with a complete HTTP response and then holds the
// connection open for `linger` without closing it.
fn lingering_responder(
    body: String,
    linger: Duration,
    split_body: bool,
) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 1024];
        let _read = stream.read(&mut buf);
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).expect("write head");
        if split_body {
            let (first, rest) = body.as_bytes().split_at(body.len() / 2);
            stream.write_all(first).expect("write first half");
            stream.flush().expect("flush");
            std::thread::sleep(Duration::from_millis(200));
            stream.write_all(rest).expect("write rest");
        } else {
            stream.write_all(body.as_bytes()).expect("write body");
        }
        stream.flush().expect("flush");
        std::thread::sleep(linger);
    });
    (port, handle)
}

#[test]
fn a_sibling_that_never_closes_the_socket_is_still_identified_within_budget() {
    let ours = InstallId::ephemeral();
    let (port, handle) =
        lingering_responder(whoami_body(48217, &ours), Duration::from_secs(2), false);

    let started = Instant::now();
    let verdict = probe_identity(port, &ours);
    let elapsed = started.elapsed();

    assert!(
        matches!(verdict, PeerIdentity::Ours(_)),
        "a complete body identifies the peer even though the socket stays open: {verdict:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1500),
        "the probe returned as soon as the declared body arrived, not at the read timeout: \
         {elapsed:?}"
    );
    handle.join().expect("responder");
}

#[test]
fn a_body_that_arrives_in_two_writes_is_read_to_its_declared_length() {
    let ours = InstallId::ephemeral();
    let foreign = InstallId::ephemeral();
    let (port, handle) =
        lingering_responder(whoami_body(48217, &foreign), Duration::from_secs(2), true);

    let verdict = probe_identity(port, &ours);
    assert!(
        matches!(&verdict, PeerIdentity::Foreign(who) if who.install_id.same_install(&foreign)),
        "a body split across writes is assembled to its content-length: {verdict:?}"
    );
    handle.join().expect("responder");
}

#[test]
fn a_listener_that_answers_nothing_is_unknown_not_unreachable() {
    let ours = InstallId::ephemeral();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let mut held = Vec::new();
        for _ in 0..2 {
            if let Ok((stream, _)) = listener.accept() {
                held.push(stream);
            }
        }
        std::thread::sleep(Duration::from_secs(1));
        drop(held);
    });

    let verdict = probe_identity(port, &ours);
    assert!(
        matches!(verdict, PeerIdentity::Unknown),
        "something is listening but it is not a bridge: {verdict:?}"
    );
    handle.join().expect("listener");
}

#[test]
fn a_closed_port_is_unreachable() {
    let ours = InstallId::ephemeral();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    assert!(matches!(
        probe_identity(port, &ours),
        PeerIdentity::Unreachable
    ));
}
