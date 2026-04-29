use systemprompt_cowork::integration::{ProxyProbeState, proxy_probe};

#[test]
fn no_url_yields_unconfigured_state() {
    let health = proxy_probe::probe(None);
    assert_eq!(health.state, ProxyProbeState::Unconfigured);
    assert!(health.url.is_none());
}

#[test]
fn empty_url_yields_unconfigured_state() {
    let health = proxy_probe::probe(Some(""));
    assert_eq!(health.state, ProxyProbeState::Unconfigured);
}

#[test]
fn missing_scheme_yields_http_error() {
    let health = proxy_probe::probe(Some("localhost:8080"));
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.is_some());
}

#[test]
fn refused_connection_to_closed_port_yields_refused() {
    let health = proxy_probe::probe(Some("http://127.0.0.1:1"));
    assert!(matches!(
        health.state,
        ProxyProbeState::Refused | ProxyProbeState::HttpError
    ));
}
