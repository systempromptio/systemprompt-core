use systemprompt_bridge::gui_policy::webview::{AppOrigin, Navigation};

const ORIGINS: [AppOrigin; 2] = [AppOrigin::CUSTOM_SCHEME, AppOrigin::WEBVIEW2_HTTPS];

fn uri(source: &str) -> http::Uri {
    source.parse().expect("uri")
}

#[test]
fn each_platform_navigates_in_window_only_to_its_own_origin() {
    let custom = AppOrigin::CUSTOM_SCHEME;
    let https = AppOrigin::WEBVIEW2_HTTPS;
    for target in ["sp://app/index.html", "SP://APP/settings", "about:blank"] {
        assert_eq!(custom.navigation(target), Navigation::InApp, "{target}");
    }
    for target in [
        "https://sp.app/index.html",
        "https://SP.APP/settings",
        "about:srcdoc",
    ] {
        assert_eq!(https.navigation(target), Navigation::InApp, "{target}");
    }
    assert_eq!(
        custom.navigation("https://sp.app/index.html"),
        Navigation::External,
        "the public sp.app domain is never the app where sp:// is served"
    );
    assert_eq!(
        https.navigation("sp://app/index.html"),
        Navigation::External,
        "an sp:// URL is not served by WebView2"
    );
}

#[test]
fn look_alike_and_foreign_hosts_never_navigate_in_window() {
    for origin in ORIGINS {
        for target in [
            "https://sp.app.evil.example/index.html",
            "https://sp.application.example/",
            "https://sp.app:8443/index.html",
            "https://evil.example/?u=https://sp.app",
            "https://user@sp.app/",
            "https://user:pw@sp.app/",
            "sp://user@app/index.html",
            "https://user@sp.app.evil.example/",
            "http://sp.app/index.html",
            "sp://app.evil/index.html",
            "sp://app:1/index.html",
            "about:config",
            "javascript:alert(1)",
            "not a url",
        ] {
            assert_eq!(
                origin.navigation(target),
                Navigation::External,
                "{origin:?} {target}"
            );
        }
    }
}

#[test]
fn ipc_is_accepted_only_from_the_platform_origin() {
    assert!(AppOrigin::CUSTOM_SCHEME.admits_ipc(&uri("sp://app/index.html")));
    assert!(AppOrigin::WEBVIEW2_HTTPS.admits_ipc(&uri("https://sp.app/index.html")));
    assert!(!AppOrigin::CUSTOM_SCHEME.admits_ipc(&uri("https://sp.app/index.html")));
    assert!(!AppOrigin::WEBVIEW2_HTTPS.admits_ipc(&uri("sp://app/index.html")));
    for origin in ORIGINS {
        for source in [
            "https://sp.app.evil.example/index.html",
            "https://evil.example/",
            "https://sp.app:444/index.html",
            "https://user@sp.app/index.html",
            "http://sp.app/index.html",
            "/index.html",
        ] {
            assert!(!origin.admits_ipc(&uri(source)), "{origin:?} {source}");
        }
    }
}

#[test]
fn the_index_url_is_served_from_the_origin_itself() {
    for origin in ORIGINS {
        let index = origin.index_url();
        assert_eq!(origin.navigation(&index), Navigation::InApp, "{index}");
        assert!(origin.admits_ipc(&uri(&index)), "{index}");
    }
    assert_eq!(AppOrigin::CUSTOM_SCHEME.index_url(), "sp://app/index.html");
    assert_eq!(
        AppOrigin::WEBVIEW2_HTTPS.index_url(),
        "https://sp.app/index.html"
    );
}
