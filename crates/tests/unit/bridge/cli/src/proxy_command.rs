use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::proxy::ProxyRole;

// Why: `proxy` blocks on Ctrl-C once it is the serving process, so it is only
// callable from a test in the branch that returns — and the call is fenced
// behind a timeout so a regression fails the test instead of hanging it.
fn run_proxy_command_with_timeout(budget: Duration) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let args = vec!["systemprompt-bridge".to_owned(), "proxy".to_owned()];
        let code = systemprompt_bridge::cli::run_with_args(&args);
        let _ = tx.send(format!("{code:?}"));
    });
    rx.recv_timeout(budget).ok()
}

#[test]
fn the_proxy_command_stands_down_when_a_sibling_already_serves() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let serving = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        let ProxyRole::Serving(served) = serving.proxy.role() else {
            panic!(
                "the fixture proxy must be the one serving: {:?}",
                serving.proxy.role()
            );
        };
        let port = served.port;

        let code = run_proxy_command_with_timeout(Duration::from_secs(20))
            .expect("the command returns instead of taking over the port");
        assert_eq!(
            code,
            format!("{:?}", ExitCode::SUCCESS),
            "finding our own proxy already up is the wanted outcome, not a failure"
        );

        let record = systemprompt_bridge::proxy::portfile::read(serving.install_id())
            .expect("the record survives");
        assert_eq!(
            record.port, port,
            "the command must not repoint the recorded port at a second listener"
        );
    });
}
