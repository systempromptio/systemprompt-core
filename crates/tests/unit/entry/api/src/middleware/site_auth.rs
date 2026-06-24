//! Post-login redirect builder for the site-auth gate.

use axum::http::Uri;
use systemprompt_api::services::middleware::login_redirect;

#[test]
fn preserves_query_string() {
    let uri: Uri = "/bridge-auth/device-link?redirect=http://127.0.0.1:9000/cb"
        .parse()
        .unwrap();
    let out = login_redirect("/admin/login", &uri);
    assert_eq!(
        out,
        "/admin/login?redirect=%2Fbridge-auth%2Fdevice-link%3Fredirect%3Dhttp%3A%2F%2F127.0.0.1%3A9000%2Fcb"
    );
}

#[test]
fn path_only_round_trips() {
    let uri: Uri = "/admin/agents".parse().unwrap();
    let out = login_redirect("/admin/login", &uri);
    assert_eq!(out, "/admin/login?redirect=%2Fadmin%2Fagents");
}

#[test]
fn nested_redirect_is_fully_encoded() {
    let uri: Uri = "/bridge-auth/device-link?redirect=x&evil=1"
        .parse()
        .unwrap();
    let out = login_redirect("/admin/login", &uri);
    // The whole original target is one encoded value — its `&` is escaped, so it
    // cannot inject a sibling query param into the login URL.
    assert!(!out.contains("&evil"));
    assert!(out.contains("%26evil%3D1"));
}
