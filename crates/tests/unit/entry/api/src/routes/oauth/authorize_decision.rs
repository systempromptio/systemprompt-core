//! The `/authorize` decision as a matrix: prompt, max_age, the verified reauth
//! time, the browser sign-in, remembered consent and the silence policy in, one
//! [`Decision`] out. Also the interaction floor for native redirects and the
//! claims a cookie token must carry to count as a browser session.

use std::collections::BTreeMap;

use systemprompt_api::routes::oauth::endpoints::authorize::AuthorizeQuery;
use systemprompt_api::routes::oauth::endpoints::authorize::decision::{
    Decision, REAUTH_WINDOW_SECS, Standing, decide, is_native_redirect, silent_allowed,
};
use systemprompt_api::routes::oauth::endpoints::authorize::session::{
    BrowserSignIn, session_subject,
};
use systemprompt_identifiers::{ClientId, PluginId, SessionId, UserId};
use systemprompt_manifest::profile::AuthorizeConfig;
use systemprompt_models::auth::{
    ActClaim, CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims,
    TokenInteraction, TokenType, UserType,
};

const NOW: i64 = 1_800_000_000;

fn query(prompt: Option<&str>, max_age: Option<i64>) -> AuthorizeQuery {
    AuthorizeQuery {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("user".to_string()),
        state: Some("state-token-with-enough-entropy-000000".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        response_mode: None,
        display: None,
        prompt: prompt.map(str::to_owned),
        max_age,
        ui_locales: None,
        resource: None,
        login_hint: None,
        reauth_since: None,
    }
}

fn signed_in(ago_secs: i64) -> BrowserSignIn {
    BrowserSignIn {
        user_id: UserId::new("user-1"),
        session_id: SessionId::new("sess-1"),
        auth_time: NOW - ago_secs,
        account: "ada@example.com".to_string(),
    }
}

struct Case {
    name: &'static str,
    prompt: Option<&'static str>,
    max_age: Option<i64>,
    reauth_since: Option<i64>,
    signed_in_ago: Option<i64>,
    covered: bool,
    silent: bool,
    expect: Decision,
}

const fn case(name: &'static str, expect: Decision) -> Case {
    Case {
        name,
        prompt: None,
        max_age: None,
        reauth_since: None,
        signed_in_ago: Some(60),
        covered: true,
        silent: true,
        expect,
    }
}

#[test]
fn decision_matrix() {
    let cases = [
        case("session, consent, silent", Decision::IssueCode),
        Case {
            covered: false,
            ..case("no remembered consent", Decision::ConsentPage)
        },
        Case {
            silent: false,
            ..case("consent but no silent issuance", Decision::ConsentPage)
        },
        Case {
            covered: false,
            silent: false,
            ..case("neither consent nor silence", Decision::ConsentPage)
        },
        Case {
            prompt: Some("consent"),
            ..case("prompt=consent always asks", Decision::ConsentPage)
        },
        Case {
            signed_in_ago: None,
            ..case("no session", Decision::SignIn)
        },
        Case {
            signed_in_ago: None,
            covered: false,
            silent: false,
            ..case("no session, nothing remembered", Decision::SignIn)
        },
        Case {
            prompt: Some("login"),
            ..case("prompt=login", Decision::SignIn)
        },
        Case {
            prompt: Some("select_account"),
            ..case("prompt=select_account", Decision::SignIn)
        },
        Case {
            prompt: Some("login consent"),
            ..case("prompt=login consent", Decision::SignIn)
        },
        Case {
            max_age: Some(59),
            ..case("sign-in older than max_age", Decision::SignIn)
        },
        Case {
            max_age: Some(60),
            ..case("sign-in exactly max_age old", Decision::IssueCode)
        },
        Case {
            max_age: Some(3600),
            ..case("sign-in within max_age", Decision::IssueCode)
        },
        Case {
            max_age: Some(0),
            signed_in_ago: Some(0),
            ..case("max_age=0 always asks", Decision::SignIn)
        },
        Case {
            max_age: Some(-5),
            ..case("negative max_age is never satisfied", Decision::SignIn)
        },
        Case {
            prompt: Some("none"),
            ..case("prompt=none, all satisfied", Decision::IssueCode)
        },
        Case {
            prompt: Some("none"),
            signed_in_ago: None,
            ..case("prompt=none, no session", Decision::LoginRequired)
        },
        Case {
            prompt: Some("none"),
            max_age: Some(10),
            ..case("prompt=none, stale sign-in", Decision::LoginRequired)
        },
        Case {
            prompt: Some("none"),
            max_age: Some(0),
            ..case("prompt=none, max_age=0", Decision::LoginRequired)
        },
        Case {
            prompt: Some("none"),
            covered: false,
            ..case("prompt=none, no consent", Decision::ConsentRequired)
        },
        Case {
            prompt: Some("none"),
            covered: false,
            silent: false,
            ..case(
                "prompt=none, no consent, not silent",
                Decision::ConsentRequired,
            )
        },
        Case {
            prompt: Some("none"),
            silent: false,
            ..case(
                "prompt=none, consent, not silent",
                Decision::InteractionRequired,
            )
        },
        Case {
            prompt: Some("none consent"),
            ..case(
                "none beside consent never issues",
                Decision::ConsentRequired,
            )
        },
        Case {
            prompt: Some("none login"),
            ..case("none beside login never issues", Decision::LoginRequired)
        },
        Case {
            prompt: Some("passkey"),
            ..case("prompt=passkey with a session", Decision::PasskeyForm)
        },
        Case {
            prompt: Some("passkey"),
            signed_in_ago: None,
            ..case("prompt=passkey without a session", Decision::PasskeyForm)
        },
        Case {
            prompt: Some("passkey login"),
            reauth_since: Some(NOW - 5),
            ..case("prompt=passkey beats the marker", Decision::PasskeyForm)
        },
    ];
    for c in cases {
        let sign_in = c.signed_in_ago.map(signed_in);
        let got = decide(
            &query(c.prompt, c.max_age),
            c.reauth_since,
            sign_in.as_ref(),
            Standing {
                consent_covered: c.covered,
                silent_allowed: c.silent,
            },
            NOW,
        );
        assert_eq!(got, c.expect, "{}", c.name);
    }
}

#[test]
fn a_return_from_the_login_page_is_never_sent_to_sign_in_again() {
    let marker = NOW - 30;
    let cases = [
        Case {
            reauth_since: Some(marker),
            signed_in_ago: None,
            ..case("returned without a session", Decision::LoginRequired)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(31),
            ..case("sign-in predates the marker", Decision::LoginRequired)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(3600),
            prompt: Some("login"),
            ..case(
                "prompt=login, still the old sign-in",
                Decision::LoginRequired,
            )
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(30),
            ..case("sign-in at the marker", Decision::IssueCode)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            prompt: Some("login"),
            ..case("prompt=login, fresh sign-in", Decision::IssueCode)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            prompt: Some("select_account"),
            covered: false,
            ..case("fresh sign-in, no consent", Decision::ConsentPage)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            prompt: Some("login consent"),
            ..case("fresh sign-in, prompt=consent", Decision::ConsentPage)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            max_age: Some(0),
            ..case("max_age=0, fresh sign-in", Decision::IssueCode)
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            max_age: Some(5),
            ..case(
                "fresh sign-in already past max_age",
                Decision::LoginRequired,
            )
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            max_age: Some(-1),
            ..case("negative max_age on return", Decision::LoginRequired)
        },
        Case {
            reauth_since: Some(NOW + 1),
            signed_in_ago: Some(0),
            ..case("marker in the future", Decision::LoginRequired)
        },
        Case {
            reauth_since: Some(NOW - REAUTH_WINDOW_SECS),
            signed_in_ago: Some(5),
            ..case("marker at the edge of its window", Decision::IssueCode)
        },
        Case {
            reauth_since: Some(NOW - REAUTH_WINDOW_SECS - 1),
            signed_in_ago: Some(5),
            ..case("marker older than its window", Decision::LoginRequired)
        },
        Case {
            reauth_since: Some(0),
            signed_in_ago: Some(5),
            prompt: Some("login"),
            ..case(
                "an ancient marker cannot waive prompt=login",
                Decision::LoginRequired,
            )
        },
        Case {
            reauth_since: Some(marker),
            signed_in_ago: Some(10),
            prompt: Some("none"),
            silent: false,
            ..case(
                "prompt=none on return, not silent",
                Decision::InteractionRequired,
            )
        },
    ];
    for c in cases {
        let sign_in = c.signed_in_ago.map(signed_in);
        let got = decide(
            &query(c.prompt, c.max_age),
            c.reauth_since,
            sign_in.as_ref(),
            Standing {
                consent_covered: c.covered,
                silent_allowed: c.silent,
            },
            NOW,
        );
        assert_eq!(got, c.expect, "{}", c.name);
    }
}

#[test]
fn without_a_verified_marker_prompt_login_and_max_age_zero_ask_for_a_sign_in() {
    let five_minutes_old = signed_in(300);
    for (prompt, max_age) in [(Some("login"), None), (None, Some(0))] {
        let got = decide(
            &query(prompt, max_age),
            None,
            Some(&five_minutes_old),
            Standing {
                consent_covered: true,
                silent_allowed: true,
            },
            NOW,
        );
        assert_eq!(got, Decision::SignIn, "{prompt:?} {max_age:?}");
    }
}

#[test]
fn only_a_non_loopback_https_redirect_is_not_native() {
    for web in [
        "https://app.example.com/callback",
        "https://app.example.com:8443/cb?x=1",
    ] {
        assert!(!is_native_redirect(web), "{web}");
    }
    for native in [
        "http://127.0.0.1/callback",
        "http://127.0.0.1:53211/callback",
        "http://[::1]:9000/cb",
        "http://localhost/cb",
        "http://LOCALHOST:1/cb",
        "https://localhost/cb",
        "https://127.0.0.1/cb",
        "https://[::1]/cb",
        "http://intranet.example.com/cb",
        "com.example.app:/oauth2redirect",
        "dynwork://auth/callback",
        "/relative/callback",
        "not a url",
        "",
    ] {
        assert!(is_native_redirect(native), "{native}");
    }
}

#[test]
fn silence_needs_a_declared_client_and_a_web_redirect_or_the_opt_in() {
    let default = AuthorizeConfig::default();
    let opted_in = AuthorizeConfig {
        silent_native_clients: true,
    };
    let web = "https://app.example.com/callback";
    let loopback = "http://127.0.0.1:53211/callback";
    let scheme = "dynwork://auth/callback";

    assert!(!default.silent_native_clients);
    assert!(silent_allowed(true, web, default));
    assert!(!silent_allowed(true, loopback, default));
    assert!(!silent_allowed(true, scheme, default));
    assert!(silent_allowed(true, loopback, opted_in));
    assert!(silent_allowed(true, scheme, opted_in));
    for redirect in [web, loopback, scheme] {
        assert!(!silent_allowed(false, redirect, default), "{redirect}");
        assert!(!silent_allowed(false, redirect, opted_in), "{redirect}");
    }
}

fn web_login_claims() -> JwtClaims {
    JwtClaims {
        registered: RegisteredClaims {
            sub: "user-1".to_string(),
            iat: NOW,
            exp: NOW + 3600,
            nbf: Some(NOW),
            iss: "https://issuer.example".to_string(),
            aud: JwtAudience::standard(),
            jti: "jti-1".to_string(),
        },
        custom: CustomClaims {
            scope: vec![Permission::User],
            username: "ada".to_string(),
            email: "ada@example.com".to_string(),
            user_type: UserType::User,
            roles: vec!["user".to_string()],
            attributes: BTreeMap::new(),
            client_id: None,
            token_type: TokenType::Bearer,
            auth_time: NOW - 60,
            session_id: Some(SessionId::new("sess-1")),
            rate_limit_tier: Some(RateLimitTier::User),
            plugin_id: None,
            act: None,
            purpose: None,
            interaction: None,
            browser_session: true,
            ui_session: false,
        },
    }
}

fn refused(edit: impl FnOnce(&mut JwtClaims)) -> bool {
    let mut claims = web_login_claims();
    edit(&mut claims);
    session_subject(&claims).is_none()
}

#[test]
fn the_web_login_token_names_its_user_and_session() {
    let (user, session) = session_subject(&web_login_claims()).expect("a browser session");
    assert_eq!(user.as_str(), "user-1");
    assert_eq!(session.as_str(), "sess-1");

    let mut admin = web_login_claims();
    admin.custom.user_type = UserType::Admin;
    admin.custom.interaction = Some(TokenInteraction::Interactive);
    assert!(session_subject(&admin).is_some());
}

#[test]
fn any_other_token_in_the_cookie_is_not_a_browser_session() {
    assert!(refused(|c| c.custom.browser_session = false));
    assert!(refused(|c| c.custom.client_id = Some(ClientId::web())));
    assert!(refused(|c| c.custom.ui_session = true));
    assert!(refused(|c| c.custom.purpose = Some("download".to_string())));
    assert!(refused(|c| c.custom.plugin_id = Some(PluginId::new("p"))));
    assert!(refused(|c| {
        c.custom.interaction = Some(TokenInteraction::NonInteractive);
    }));
    assert!(refused(|c| {
        c.custom.act = Some(ActClaim {
            iss: "https://issuer.example".to_string(),
            sub: "agent-1".to_string(),
            act: Box::new(None),
        });
    }));
    assert!(refused(|c| c.custom.session_id = None));
    assert!(refused(|c| c.registered.sub = String::new()));
    for not_a_person in [
        UserType::Anon,
        UserType::Service,
        UserType::A2a,
        UserType::Mcp,
    ] {
        assert!(
            refused(|c| c.custom.user_type = not_a_person),
            "{not_a_person:?}"
        );
    }
}
