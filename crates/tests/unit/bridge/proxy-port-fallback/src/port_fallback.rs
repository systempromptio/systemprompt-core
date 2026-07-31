//! The WSL2/Windows collision, reduced to one process.
//!
//! `start_default` publishes its handle in a process-global `OnceLock`, so a
//! process can only demonstrate one outcome. This crate exists to own the
//! "default port was taken" outcome; `proxy-module` owns the ordinary one.

use systemprompt_bridge::proxy::{self, StartOutcome};

#[test]
fn a_taken_default_port_moves_the_proxy_instead_of_failing() {
    let temp = tempfile::tempdir().expect("config tempdir");

    // Stand in for the other machine's bridge (or, in the real bug, WSL2's
    // relay mirroring a Linux bind onto the Windows loopback).
    let squatter = std::net::TcpListener::bind(("127.0.0.1", proxy::DEFAULT_PROXY_PORT));
    let Ok(squatter) = squatter else {
        eprintln!("skipping: port {} is already in use", proxy::DEFAULT_PROXY_PORT);
        return;
    };

    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let outcome = proxy::start_default();
        let StartOutcome::Started(handle) = outcome else {
            panic!("a taken default port must not stop the proxy: {outcome:?}");
        };

        assert_ne!(
            handle.port,
            proxy::DEFAULT_PROXY_PORT,
            "the squatter still holds the default port"
        );
        assert!(
            proxy::candidate_ports().contains(&handle.port),
            "fell outside the candidate range: {}",
            handle.port
        );

        // The whole point of moving: everything downstream has to be able to
        // find the new port, including processes that are not this one.
        assert_eq!(
            proxy::loopback_origin(),
            format!("http://127.0.0.1:{}", handle.port)
        );
        let record = proxy::portfile::read().expect("the fallback port is recorded on disk");
        assert_eq!(record.port, handle.port);
    });

    drop(squatter);
}
