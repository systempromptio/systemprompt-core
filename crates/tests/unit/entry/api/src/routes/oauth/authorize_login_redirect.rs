//! `login_page_redirect_target` — the security.login_page_url redirect
//! decision: carry the original authorize query to the deployment login page,
//! honour the `prompt=passkey` opt-out, and refuse non-http(s) targets.

use systemprompt_api::routes::oauth::endpoints::authorize::{
    AuthorizeQuery, login_page_redirect_target,
};
use systemprompt_identifiers::ClientId;

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
    }
}

#[test]
fn redirects_to_login_page_carrying_the_original_query() {
    let target = login_page_redirect_target("http://localhost:8081/admin/login", &query())
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
fn prompt_passkey_opts_back_into_the_builtin_form() {
    let mut params = query();
    params.prompt = Some("passkey".to_string());

    assert!(login_page_redirect_target("http://localhost:8081/admin/login", &params).is_none());
}

#[test]
fn absent_optional_params_are_omitted_from_the_query() {
    let mut params = query();
    params.scope = None;
    params.resource = None;

    let target = login_page_redirect_target("https://login.example.com/signin", &params)
        .expect("configured URL redirects");
    assert!(!target.contains("scope="));
    assert!(!target.contains("resource="));
}

#[test]
fn non_http_login_page_url_is_refused() {
    for bad in ["/admin/login", "javascript:alert(1)", "ftp://x", ""] {
        assert!(
            login_page_redirect_target(bad, &query()).is_none(),
            "{bad:?} must not redirect"
        );
    }
}
