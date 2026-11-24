//! What `/authorize` does with a validated request: a pure function of the
//! request, the browser sign-in, and the consent and silence policy.
//!
//! A sign-in is usable when nothing asks for a new one: `prompt` names
//! neither `login` nor `select_account`, and the sign-in is no older than
//! `max_age` (`max_age=0` always asks). A request that returns from the login
//! page carries the time the endpoint sent it there, which the handler passes
//! in only once the server-signed marker has verified for this very request.
//! It is satisfied only by a sign-in at or after that time, within
//! [`REAUTH_WINDOW_SECS`] of it, and is otherwise answered `login_required`
//! rather than sent to sign in a second time.
//!
//! A usable sign-in yields a code with no page only when consent is remembered
//! and the client may be issued to silently. A `redirect_uri` that is not
//! `https` to a non-loopback host cannot prove which client receives the code
//! (RFC 8252 §8.6), so [`silent_allowed`] is false for it unless the operator
//! opted in with `security.authorize.silent_native_clients`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::AuthorizeConfig;
use systemprompt_oauth::repository::REAUTH_MARKER_TTL_SECS;
use systemprompt_oauth::services::validation::registration_redirect::is_loopback;

use super::AuthorizeQuery;
use super::session::BrowserSignIn;

pub const REAUTH_WINDOW_SECS: i64 = REAUTH_MARKER_TTL_SECS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    PasskeyForm,
    SignIn,
    LoginRequired,
    ConsentRequired,
    InteractionRequired,
    ConsentPage,
    IssueCode,
}

#[derive(Debug, Clone, Copy, Default)]
struct Prompt {
    none: bool,
    login: bool,
    consent: bool,
    passkey: bool,
}

impl Prompt {
    fn of(params: &AuthorizeQuery) -> Self {
        let mut prompt = Self::default();
        for value in params
            .prompt
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
        {
            match value {
                "none" => prompt.none = true,
                "login" | "select_account" => prompt.login = true,
                "consent" => prompt.consent = true,
                "passkey" => prompt.passkey = true,
                _ => {},
            }
        }
        prompt
    }
}

fn sign_in_usable(
    params: &AuthorizeQuery,
    reauth_since: Option<i64>,
    sign_in: &BrowserSignIn,
    prompt: Prompt,
    now: i64,
) -> bool {
    let age = now.saturating_sub(sign_in.auth_time);
    reauth_since.map_or_else(
        || {
            let asked = prompt.login || params.max_age == Some(0);
            let stale = params.max_age.is_some_and(|max_age| age > max_age);
            !asked && !stale
        },
        |since| {
            let marker_current = since <= now && now.saturating_sub(since) <= REAUTH_WINDOW_SECS;
            let within_max_age = params
                .max_age
                .is_none_or(|max_age| max_age == 0 || age <= max_age);
            marker_current && sign_in.auth_time >= since && within_max_age
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub consent_covered: bool,
    pub silent_allowed: bool,
}

#[must_use]
pub fn decide(
    params: &AuthorizeQuery,
    reauth_since: Option<i64>,
    sign_in: Option<&BrowserSignIn>,
    standing: Standing,
    now: i64,
) -> Decision {
    let Standing {
        consent_covered,
        silent_allowed,
    } = standing;
    let prompt = Prompt::of(params);
    if prompt.passkey {
        return Decision::PasskeyForm;
    }
    let signed_in =
        sign_in.is_some_and(|sign_in| sign_in_usable(params, reauth_since, sign_in, prompt, now));
    if !signed_in {
        return if prompt.none || reauth_since.is_some() {
            Decision::LoginRequired
        } else {
            Decision::SignIn
        };
    }
    if consent_covered && silent_allowed && !prompt.consent {
        return Decision::IssueCode;
    }
    if prompt.none {
        return if consent_covered && !prompt.consent {
            Decision::InteractionRequired
        } else {
            Decision::ConsentRequired
        };
    }
    Decision::ConsentPage
}

#[must_use]
pub fn is_web_redirect(redirect_uri: &str) -> bool {
    url::Url::parse(redirect_uri).is_ok_and(|url| url.scheme() == "https" && !is_loopback(&url))
}

#[must_use]
pub fn is_native_redirect(redirect_uri: &str) -> bool {
    !is_web_redirect(redirect_uri)
}

#[must_use]
pub fn silent_allowed(declared_client: bool, redirect_uri: &str, policy: AuthorizeConfig) -> bool {
    declared_client && (policy.silent_native_clients || is_web_redirect(redirect_uri))
}
