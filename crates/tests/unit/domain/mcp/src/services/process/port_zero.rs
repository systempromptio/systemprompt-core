use std::process::{Child, Command};
use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::network::port::{
    cleanup_port_processes, prepare_port, wait_for_port_release_with_retry,
};
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::process::pid::{find_pid_by_port, find_process_on_port_with_name};

const MARKER_HELPER: &str = "services::process::port_zero::port_zero_marker_helper";
const SERVICE: &str = "port-zero-test";

#[test]
#[ignore = "re-executed as a child process by the port-zero tests"]
fn port_zero_marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(30));
}

struct Sleeper(Child);

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_sleeper() -> Sleeper {
    Sleeper(
        Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep"),
    )
}

#[test]
fn port_zero_resolves_to_no_pid() {
    assert_eq!(find_pid_by_port(0).expect("lookup ok"), None);
    assert_eq!(
        ProcessService::find_pid_by_port(0).expect("lookup ok"),
        None
    );
}

#[test]
fn port_zero_resolves_to_no_named_pid() {
    let me = std::env::current_exe().expect("exe");
    let name = me.file_name().expect("name").to_string_lossy().into_owned();

    assert_eq!(
        find_process_on_port_with_name(0, &name).expect("lookup ok"),
        None
    );
    assert_eq!(
        ProcessService::find_process_on_port_with_name(0, &name).expect("lookup ok"),
        None
    );
}

#[tokio::test]
async fn port_zero_cleanup_signals_no_process() {
    let mut unrelated = spawn_sleeper();
    let mut marked = systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, SERVICE);
    let service = ServiceName::new(SERVICE);

    cleanup_port_processes(0, &service)
        .await
        .expect("port 0 cleanup is a no-op");
    prepare_port(0, &service)
        .await
        .expect("port 0 preparation is a no-op");
    wait_for_port_release_with_retry(0, &service, 2)
        .await
        .expect("port 0 is never held");

    assert!(
        unrelated.0.try_wait().expect("try_wait").is_none(),
        "an unrelated process must survive a port-0 cleanup"
    );
    assert!(
        marked.child.try_wait().expect("try_wait").is_none(),
        "even our own marked child is never selected through port 0"
    );
}
