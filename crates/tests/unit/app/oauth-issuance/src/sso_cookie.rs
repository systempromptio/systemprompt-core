use systemprompt_oauth_issuance::sso::{
    BINDER_COOKIE_NAME, CookieSameSite, CookieSpec, binder_from_cookie_header, sanitize_return_to,
};

#[test]
fn binder_header_value_on_https() {
    let spec = CookieSpec::binder(
        "sso_abc".to_owned(),
        "/sso/callback",
        600,
        CookieSameSite::None,
        "https://app.example.com",
    );
    assert_eq!(spec.name, BINDER_COOKIE_NAME);
    assert!(spec.secure);
    assert_eq!(
        spec.header_value(),
        "sp_sso_bind=sso_abc; Path=/sso/callback; HttpOnly; SameSite=None; Max-Age=600; Secure"
    );
}

#[test]
fn secure_dropped_on_plain_http_and_same_site_none_degrades() {
    let spec = CookieSpec::binder(
        "v".to_owned(),
        "/cb",
        600,
        CookieSameSite::None,
        "http://localhost:8080",
    );
    assert!(!spec.secure);
    assert_eq!(spec.same_site, CookieSameSite::Lax);
    assert_eq!(
        spec.header_value(),
        "sp_sso_bind=v; Path=/cb; HttpOnly; SameSite=Lax; Max-Age=600"
    );
}

#[test]
fn clear_empties_and_expires() {
    let spec = CookieSpec::binder(
        "v".to_owned(),
        "/cb",
        600,
        CookieSameSite::Lax,
        "https://x.test",
    );
    let cleared = spec.clear();
    assert_eq!(cleared.value, "");
    assert_eq!(cleared.max_age, 0);
    assert_eq!(cleared.path, "/cb");
}

#[test]
fn binder_read_from_cookie_header() {
    assert_eq!(
        binder_from_cookie_header("a=1; sp_sso_bind=sso_xyz; b=2"),
        Some("sso_xyz")
    );
    assert_eq!(binder_from_cookie_header("sp_sso_bind="), None);
    assert_eq!(binder_from_cookie_header("other=1"), None);
}

#[test]
fn return_to_must_be_same_origin_relative() {
    assert_eq!(sanitize_return_to("/admin?tab=1"), Some("/admin?tab=1"));
    assert_eq!(sanitize_return_to("//evil.test/x"), None);
    assert_eq!(sanitize_return_to("/\\evil.test"), None);
    assert_eq!(sanitize_return_to("https://evil.test"), None);
    assert_eq!(sanitize_return_to("/a\nb"), None);
}
