//! `login_page_redirect_target` — the security.login_page_url redirect:
//! carry the authorization request, `prompt`, `max_age` and `login_hint` to
//! the deployment login page together with the way back (`redirect`, the
//! request's own path and query with the server-signed `reauth_since` marker
//! in place of any it carried), and refuse
//! non-http(s) targets and a way back that is not a same-origin path.

use std::collections::BTreeMap;

use systemprompt_api::routes::oauth::endpoints::authorize::{
    AuthorizeQuery, login_page_redirect_target,
};
use systemprompt_identifiers::ClientId;

const LOGIN: &str = "http://localhost:8081/admin/login";
const RETURN_PATH: &str =
    "/api/v1/core/oauth/authorize?response_type=code&client_id=sp_test_client&prompt=login";
const MARKER: &str = "1800000000.0123abcd";

fn query() -> AuthorizeQuery {
    AuthorizeQuery {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("mcp".to_string()),
        state: Some("state-token-with-enough-entropy-000000".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        response_mode: None,
        display: None,
        prompt: None,
        max_age: None,
        ui_locales: None,
        resource: Some("https://example.com/api/v1/mcp/odoo/mcp".to_string()),
        login_hint: None,
        reauth_since: None,
    }
}

fn pairs(target: &str) -> BTreeMap<String, String> {
    url::Url::parse(target)
        .expect("target is a URL")
        .query_pairs()
        .into_owned()
        .collect()
}

#[test]
fn redirects_to_login_page_carrying_the_original_query() {
    let target = login_page_redirect_target(LOGIN, &query(), RETURN_PATH, MARKER)
        .expect("configured URL redirects");

    assert!(target.starts_with("http://localhost:8081/admin/login?"));
    for fragment in [
        "response_type=code",
        "client_id=sp_test_client",
        "redirect_uri=https%3A%2F%2Fexample.com%2Fcallback",
        "scope=mcp",
        "state=state-token-with-enough-entropy-000000",
        "code_challenge=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "code_challenge_method=S256",
        "resource=https%3A%2F%2Fexample.com%2Fapi%2Fv1%2Fmcp%2Fodoo%2Fmcp",
    ] {
        assert!(target.contains(fragment), "missing {fragment} in {target}");
    }
}

#[test]
fn the_way_back_is_this_request_with_the_reauth_marker_appended() {
    let target = login_page_redirect_target(LOGIN, &query(), RETURN_PATH, MARKER)
        .expect("configured URL redirects");

    let redirect = &pairs(&target)["redirect"];
    assert_eq!(redirect, &format!("{RETURN_PATH}&reauth_since={MARKER}"));
    assert!(redirect.starts_with('/') && !redirect.starts_with("//"));
    assert!(
        target.contains("redirect=%2Fapi%2Fv1%2Fcore%2Foauth%2Fauthorize%3F"),
        "the way back is one url-encoded value: {target}"
    );
}

#[test]
fn a_request_path_without_a_query_gets_the_marker_as_its_query() {
    let target = login_page_redirect_target(LOGIN, &query(), "/authorize", MARKER)
        .expect("configured URL redirects");

    assert_eq!(
        pairs(&target)["redirect"],
        format!("/authorize?reauth_since={MARKER}")
    );
}

#[test]
fn prompt_max_age_and_login_hint_reach_the_login_page_when_present() {
    let mut params = query();
    params.prompt = Some("login consent".to_string());
    params.max_age = Some(0);
    params.login_hint = Some("ada@example.com".to_string());

    let target = login_page_redirect_target(LOGIN, &params, RETURN_PATH, MARKER)
        .expect("configured URL redirects");
    let sent = pairs(&target);
    assert_eq!(sent["prompt"], "login consent");
    assert_eq!(sent["max_age"], "0");
    assert_eq!(sent["login_hint"], "ada@example.com");

    let plain = pairs(
        &login_page_redirect_target(LOGIN, &query(), RETURN_PATH, MARKER)
            .expect("configured URL redirects"),
    );
    for absent in ["prompt", "max_age", "login_hint"] {
        assert!(!plain.contains_key(absent), "{absent} must be omitted");
    }
}

#[test]
fn absent_optional_params_are_omitted_from_the_query() {
    let mut params = query();
    params.scope = None;
    params.resource = None;

    let target = login_page_redirect_target(
        "https://login.example.com/signin",
        &params,
        RETURN_PATH,
        MARKER,
    )
    .expect("configured URL redirects");
    let sent = pairs(&target);
    assert!(!sent.contains_key("scope"));
    assert!(!sent.contains_key("resource"));
}

#[test]
fn a_login_page_url_with_its_own_query_keeps_it() {
    let target = login_page_redirect_target(
        "https://login.example.com/signin?tenant=acme",
        &query(),
        RETURN_PATH,
        MARKER,
    )
    .expect("configured URL redirects");

    assert!(target.starts_with("https://login.example.com/signin?tenant=acme&response_type=code"));
}

#[test]
fn non_http_login_page_url_is_refused() {
    for bad in ["/admin/login", "javascript:alert(1)", "ftp://x", ""] {
        assert!(
            login_page_redirect_target(bad, &query(), RETURN_PATH, MARKER).is_none(),
            "{bad:?} must not redirect"
        );
    }
}

#[test]
fn a_way_back_that_is_not_a_same_origin_path_is_refused() {
    for bad in [
        "//evil.example/authorize?x=1",
        "https://evil.example/authorize",
        "authorize?x=1",
        "/authorize\\@evil.example",
    ] {
        assert!(
            login_page_redirect_target(LOGIN, &query(), bad, MARKER).is_none(),
            "{bad:?} must not become a redirect target"
        );
    }
}

#[test]
fn a_marker_the_request_already_carried_is_replaced_not_repeated() {
    for (carried, expected) in [
        (
            format!("{RETURN_PATH}&reauth_since=forged"),
            format!("{RETURN_PATH}&reauth_since={MARKER}"),
        ),
        (
            "/authorize?reauth_since=1.aa".to_owned(),
            format!("/authorize?reauth_since={MARKER}"),
        ),
    ] {
        let target = login_page_redirect_target(LOGIN, &query(), &carried, MARKER)
            .expect("configured URL redirects");
        assert_eq!(pairs(&target)["redirect"], expected, "{carried}");
    }
}
