//! `PortService` against ports that are genuinely occupied while the port
//! probe itself is broken.
//!
//! Whether a holder may be reclaimed rests on `lsof` naming it; a probe that
//! is missing or answers garbage must fail closed and leave the listener
//! untouched. The PATH is swapped per test, which is safe because nextest
//! runs every test in its own process.

#![cfg(unix)]

use std::net::TcpListener;
use std::time::Duration;

use systemprompt_agent::services::agent_orchestration::OrchestrationError;
use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::subprocess::SupervisionError;

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

struct PathGuard(Option<std::ffi::OsString>);

impl Drop for PathGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

fn listener_is_alive(listener: &TcpListener) -> bool {
    std::net::TcpStream::connect_timeout(
        &listener.local_addr().expect("listener address"),
        Duration::from_secs(1),
    )
    .is_ok()
}

fn agent() -> AgentName {
    AgentName::new("occupied_port_agent")
}

#[tokio::test]
async fn malformed_lsof_output_fails_closed_without_disturbing_the_listener() {
    let (listener, port) = held_port();
    let shim = tempfile::tempdir().expect("private lsof shim directory");
    let lsof = shim.path().join("lsof");
    std::fs::write(&lsof, "#!/bin/sh\nprintf 'not-a-pid\\n'\n").expect("write lsof shim");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&lsof, std::fs::Permissions::from_mode(0o700))
        .expect("make lsof shim executable");
    let _path = PathGuard(std::env::var_os("PATH"));
    unsafe { std::env::set_var("PATH", shim.path()) };

    let error = PortService::new()
        .cleanup_port_if_needed(port, &agent())
        .await
        .expect_err("an unparseable holder identity must fail closed");

    assert!(
        error
            .to_string()
            .contains("no listening process can be identified"),
        "{error}"
    );
    assert!(
        listener_is_alive(&listener),
        "identity failure must not terminate or disturb the unverified listener"
    );
}

#[tokio::test]
async fn missing_lsof_fails_closed_and_restored_probe_still_refuses_the_listener() {
    let (listener, port) = held_port();
    let unavailable = tempfile::tempdir().expect("empty probe directory");
    let path = PathGuard(std::env::var_os("PATH"));
    unsafe { std::env::set_var("PATH", unavailable.path()) };

    let error = PortService::new()
        .cleanup_port_if_needed(port, &agent())
        .await
        .expect_err("a missing port probe must fail closed");
    assert!(
        matches!(
            error,
            OrchestrationError::Supervision(SupervisionError::Tool { tool: "lsof", .. })
        ),
        "{error:?}"
    );
    assert!(
        listener_is_alive(&listener),
        "a missing probe must not disturb the listener"
    );

    drop(path);
    let restored = PortService::new()
        .cleanup_port_if_needed(port, &agent())
        .await
        .expect_err("the restored probe must still refuse the unmarked listener");
    assert!(
        matches!(
            restored,
            OrchestrationError::PortHeldByForeignProcess { pid, .. } if pid == std::process::id()
        ),
        "{restored:?}"
    );
    assert!(
        listener_is_alive(&listener),
        "restoring a probe must not turn a bystander listener into a cleanup target"
    );
}
