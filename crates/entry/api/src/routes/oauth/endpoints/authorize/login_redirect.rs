//! The redirect to the deployment's login page (`security.login_page_url`),
//! carrying the way back.
//!
//! The page receives the authorization request's parameters, `prompt`,
//! `max_age` and `login_hint` when the client sent them, and `redirect`: the
//! path and query of this authorization request with any `reauth_since` it
//! carried replaced by the server-signed marker for this request (see
//! [`reauth_binding`]).
//! `redirect` is a same-origin path, so the page can hand it to the SSO start
//! route as it is. Once the person is signed in, the page sends the browser
//! there and the request is decided again against the new session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_oauth::repository::ReauthBinding;
use systemprompt_oauth_issuance::sso::sanitize_return_to;

use super::AuthorizeQuery;

#[must_use]
pub fn reauth_binding(params: &AuthorizeQuery) -> ReauthBinding<'_> {
    ReauthBinding {
        client_id: &params.client_id,
        response_type: &params.response_type,
        redirect_uri: params.redirect_uri.as_deref(),
        scope: params.scope.as_deref(),
        state: params.state.as_deref(),
        code_challenge: params.code_challenge.as_deref(),
        code_challenge_method: params.code_challenge_method.as_deref(),
        resource: params.resource.as_deref(),
    }
}

fn without_marker(return_path: &str) -> String {
    let Some((path, query)) = return_path.split_once('?') else {
        return return_path.to_owned();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            !pair.is_empty() && pair.split_once('=').map_or(*pair, |(key, _)| key) != "reauth_since"
        })
        .collect();
    if kept.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{}", kept.join("&"))
    }
}

#[must_use]
pub fn login_page_redirect_target(
    login_page_url: &str,
    params: &AuthorizeQuery,
    return_path: &str,
    reauth_marker: &str,
) -> Option<String> {
    match reqwest::Url::parse(login_page_url) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {},
        _ => {
            tracing::error!(
                login_page_url = %login_page_url,
                "security.login_page_url is not an absolute http(s) URL; rendering built-in form"
            );
            return None;
        },
    }
    let return_path = without_marker(return_path);
    let separator = if return_path.contains('?') { '&' } else { '?' };
    let way_back = format!(
        "{return_path}{separator}reauth_since={}",
        urlencoding::encode(reauth_marker)
    );
    let Some(way_back) = sanitize_return_to(&way_back) else {
        tracing::error!(
            "authorize request path is not a same-origin path; rendering built-in form"
        );
        return None;
    };
    let max_age = params.max_age.map(|max_age| max_age.to_string());
    let pairs = [
        ("response_type", Some(params.response_type.as_str())),
        ("client_id", Some(params.client_id.as_str())),
        ("redirect_uri", params.redirect_uri.as_deref()),
        ("scope", params.scope.as_deref()),
        ("state", params.state.as_deref()),
        ("code_challenge", params.code_challenge.as_deref()),
        (
            "code_challenge_method",
            params.code_challenge_method.as_deref(),
        ),
        ("resource", params.resource.as_deref()),
        ("prompt", params.prompt.as_deref()),
        ("max_age", max_age.as_deref()),
        ("login_hint", params.login_hint.as_deref()),
        ("redirect", Some(way_back)),
    ];
    let query = pairs
        .iter()
        .filter_map(|(key, value)| {
            value.map(|value| format!("{key}={}", urlencoding::encode(value)))
        })
        .collect::<Vec<_>>()
        .join("&");
    let separator = if login_page_url.contains('?') {
        '&'
    } else {
        '?'
    };
    Some(format!("{login_page_url}{separator}{query}"))
}
