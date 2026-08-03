// The reclaim path of `PortService`, which the refusal-oriented suites in
// `port_service_cleanup` never reach: a listener whose command line looks like
// `systemprompt admin agents run` is identified as an orphaned agent, killed,
// and the port is then observed to free up.
//
// The stand-in listener is a python process whose trailing argv carries the
// agent command pattern, so `is_agent_process` (which matches on `ps -o args`)
// classifies it exactly as it would a real orphaned agent.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use systemprompt_agent::services::agent_orchestration::port_service::{
    PortService, find_process_using_port, is_agent_process,
};

const LISTENER: &str = "import socket,sys,time\n\
s=socket.socket()\n\
s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)\n\
s.bind(('127.0.0.1',int(sys.argv[1])))\n\
s.listen()\n\
time.sleep(120)\n";

fn reserve_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

// Spawns the fake agent and waits until lsof attributes the port to it, so the
// test never races the listener's bind.
fn spawn_fake_agent(port: u16) -> Option<(Child, u32)> {
    let child = Command::new("python3")
        .arg("-c")
        .arg(LISTENER)
        .arg(port.to_string())
        .arg("systemprompt")
        .arg("admin agents run")
        .spawn()
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(Some(pid)) = find_process_using_port(port)
            && is_agent_process(pid) == Ok(true)
        {
            return Some((child, pid));
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    None
}

#[tokio::test(flavor = "multi_thread")]
async fn kill_process_on_port_reclaims_a_port_held_by_an_orphaned_agent() {
    let port = reserve_port();
    let Some((mut child, pid)) = spawn_fake_agent(port) else {
        return;
    };

    let result = PortService::new().kill_process_on_port(port).await;
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        result.expect("an agent-owned port is reclaimable"),
        "the holder at pid {pid} should have been killed"
    );
    assert!(
        find_process_using_port(port)
            .expect("probe succeeds")
            .is_none(),
        "the port is free once the holder is gone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_port_if_needed_kills_an_orphaned_agent_and_returns_ok() {
    let port = reserve_port();
    let Some((mut child, _pid)) = spawn_fake_agent(port) else {
        return;
    };

    let result = PortService::new().cleanup_port_if_needed(port).await;
    let _ = child.kill();
    let _ = child.wait();

    result.expect("cleanup reclaims an orphaned agent's port");
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_agent_ports_counts_each_port_it_reclaims() {
    let port = reserve_port();
    let Some((mut child, _pid)) = spawn_fake_agent(port) else {
        return;
    };

    let result = PortService::new()
        .cleanup_agent_ports(&[reserve_port(), port])
        .await;
    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(
        result.expect("the occupied port is reclaimed"),
        1,
        "only the occupied port is counted as cleaned"
    );
}
