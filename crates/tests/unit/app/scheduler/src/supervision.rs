//! The scheduler's port and process stops over the loader's supervision
//! module: protected database ports are never reported or signalled, a port
//! holder is stopped on a service's behalf only when it carries that service's
//! marker, and a port that stays held is a typed failure naming its holders.

use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind};
use systemprompt_manifest::services::ServiceModule;
use systemprompt_scheduler::{
    ApiListenerStop, SchedulerError, child_kind, port_holders, stop_api_listeners,
    stop_owned_port_holders, wait_for_port_free,
};

const POSTGRES_PORT: u16 = 5432;
const PGBOUNCER_PORT: u16 = 6432;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe listener");
    listener.local_addr().expect("probe address").port()
}

#[test]
fn service_modules_map_to_their_marker_kind() {
    assert_eq!(child_kind(ServiceModule::Agent), ChildKind::Agent);
    assert_eq!(child_kind(ServiceModule::Mcp), ChildKind::Mcp);
}

#[tokio::test]
async fn protected_database_ports_report_no_holders() {
    assert!(
        port_holders(POSTGRES_PORT)
            .await
            .expect("postgres port")
            .is_empty()
    );
    assert!(
        port_holders(PGBOUNCER_PORT)
            .await
            .expect("pgbouncer port")
            .is_empty()
    );
}

#[tokio::test]
async fn protected_database_ports_are_never_signalled() {
    let stopped = stop_api_listeners(POSTGRES_PORT, Duration::ZERO)
        .await
        .expect("protected port stop");
    assert!(stopped.is_empty());
}

#[tokio::test]
async fn an_unbound_port_is_free() {
    wait_for_port_free(free_port(), Duration::ZERO)
        .await
        .expect("unbound port reported free");
}

#[tokio::test]
async fn an_occupied_port_fails_naming_its_holder() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("local addr").port();

    let error = wait_for_port_free(port, Duration::ZERO)
        .await
        .expect_err("an occupied port must not be reported free");
    match error {
        SchedulerError::PortOccupied {
            port: reported,
            holders,
        } => {
            assert_eq!(reported, port);
            assert_eq!(holders, vec![std::process::id()]);
        },
        other => panic!("expected PortOccupied, got {other:?}"),
    }
}

#[cfg(unix)]
mod live_holders {
    use super::*;

    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    struct Holder(Child);

    impl Drop for Holder {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn spawn_holder(marked_as: Option<(ChildKind, &ServiceName)>) -> (Holder, u32, u16) {
        let mut command = Command::new("python3");
        command
            .args([
                "-c",
                "import socket,sys,time\ns=socket.socket()\ns.bind(('127.0.0.1',0))\nprint(s.getsockname()[1],flush=True)\ns.listen(1)\ntime.sleep(60)",
            ])
            .stdout(Stdio::piped());
        if let Some((kind, service)) = marked_as {
            subprocess::mark_child(&mut command, kind, service);
        }
        let mut child = command.spawn().expect("spawn python3 port holder");
        let stdout = child.stdout.take().expect("holder stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read holder port");
        let port = line.trim().parse::<u16>().expect("holder port number");
        let pid = child.id();
        (Holder(child), pid, port)
    }

    #[tokio::test]
    async fn a_holder_marked_for_the_service_is_stopped() {
        let service = ServiceName::new(format!("sup-owned-{}", uuid::Uuid::new_v4().simple()));
        let (_holder, pid, port) = spawn_holder(Some((ChildKind::Mcp, &service)));

        let stopped = stop_owned_port_holders(port, ChildKind::Mcp, &service, Duration::ZERO)
            .await
            .expect("owned holder stop");

        assert_eq!(stopped, vec![pid]);
        assert!(!subprocess::is_running(pid).await);
        wait_for_port_free(port, Duration::from_secs(1))
            .await
            .expect("the stopped holder released the port");
    }

    #[tokio::test]
    async fn an_unmarked_holder_is_left_running() {
        let service = ServiceName::new(format!("sup-foreign-{}", uuid::Uuid::new_v4().simple()));
        let (_holder, pid, port) = spawn_holder(None);

        let stopped = stop_owned_port_holders(port, ChildKind::Mcp, &service, Duration::ZERO)
            .await
            .expect("foreign holder stop");

        assert!(stopped.is_empty());
        assert!(subprocess::is_running(pid).await);
        assert_eq!(port_holders(port).await.expect("holders"), vec![pid]);
    }

    #[tokio::test]
    async fn a_holder_marked_for_another_service_is_left_running() {
        let other = ServiceName::new(format!("sup-other-{}", uuid::Uuid::new_v4().simple()));
        let service = ServiceName::new(format!("sup-this-{}", uuid::Uuid::new_v4().simple()));
        let (_holder, pid, port) = spawn_holder(Some((ChildKind::Mcp, &other)));

        let stopped = stop_owned_port_holders(port, ChildKind::Mcp, &service, Duration::ZERO)
            .await
            .expect("mismatched holder stop");

        assert!(stopped.is_empty());
        assert!(subprocess::is_running(pid).await);
    }

    #[tokio::test]
    async fn a_listener_stamped_as_the_api_server_is_stopped() {
        let api = subprocess::api_server_service();
        let (_holder, pid, port) = spawn_holder(Some((ChildKind::Api, &api)));

        let stopped = stop_api_listeners(port, Duration::from_secs(1))
            .await
            .expect("api listener stop");

        assert!(matches!(
            stopped.as_slice(),
            [ApiListenerStop {
                pid: stopped_pid,
                outcome: subprocess::StopOutcome::Stopped(_),
            }] if *stopped_pid == pid
        ));
        assert!(!subprocess::is_running(pid).await);
    }

    #[tokio::test]
    async fn an_unstamped_api_port_listener_is_reported_and_left_running() {
        let (_holder, pid, port) = spawn_holder(None);

        let stopped = stop_api_listeners(port, Duration::ZERO)
            .await
            .expect("foreign api port listener");

        assert_eq!(
            stopped,
            vec![ApiListenerStop {
                pid,
                outcome: subprocess::StopOutcome::NotOurs,
            }]
        );
        assert!(subprocess::is_running(pid).await);
    }

    #[tokio::test]
    async fn an_agent_marker_does_not_pass_as_the_api_server() {
        let (_holder, pid, port) =
            spawn_holder(Some((ChildKind::Agent, &subprocess::api_server_service())));

        let stopped = stop_api_listeners(port, Duration::ZERO)
            .await
            .expect("agent-marked api port listener");

        assert_eq!(stopped[0].outcome, subprocess::StopOutcome::NotOurs);
        assert!(subprocess::is_running(pid).await);
    }
}
