use systemprompt_bridge::gui_policy::webview::{Navigation, ipc_origin_allowed, navigation};

#[test]
fn app_origins_navigate_in_window() {
    for target in [
        "sp://app/index.html",
        "https://sp.app/index.html",
        "https://SP.APP/settings",
        "about:blank",
    ] {
        assert_eq!(navigation(target), Navigation::InApp, "{target}");
    }
}

#[test]
fn look_alike_and_foreign_hosts_never_navigate_in_window() {
    for target in [
        "https://sp.app.evil.example/index.html",
        "https://sp.application.example/",
        "https://sp.app:8443/index.html",
        "https://evil.example/?u=https://sp.app",
        "https://user@sp.app.evil.example/",
        "http://sp.app/index.html",
        "sp://app.evil/index.html",
        "about:config",
        "javascript:alert(1)",
        "not a url",
    ] {
        assert_eq!(navigation(target), Navigation::External, "{target}");
    }
}

#[test]
fn ipc_is_accepted_only_from_the_app_origin() {
    for source in ["sp://app/index.html", "https://sp.app/index.html"] {
        let uri: http::Uri = source.parse().expect("uri");
        assert!(ipc_origin_allowed(&uri), "{source}");
    }
    for source in [
        "https://sp.app.evil.example/index.html",
        "https://evil.example/",
        "https://sp.app:444/index.html",
        "http://sp.app/index.html",
        "/index.html",
    ] {
        let uri: http::Uri = source.parse().expect("uri");
        assert!(!ipc_origin_allowed(&uri), "{source}");
    }
}
