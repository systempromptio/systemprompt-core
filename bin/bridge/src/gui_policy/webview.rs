//! Which pages the settings webview may show in-window and which may speak
//! to the bridge over IPC.
//!
//! Both decisions are made on a parsed URL: the scheme and host must equal
//! one of the app's own origins exactly, so a look-alike host such as
//! `sp.app.example.com` is never treated as the app. These functions carry no
//! platform dependency, so they are compiled and tested on every target even
//! though the webview itself is desktop-only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const APP_ORIGINS: [(&str, &str); 2] = [("sp", "app"), ("https", "sp.app")];
const IN_APP_BLANK: [&str; 2] = ["about:blank", "about:srcdoc"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    InApp,
    External,
}

fn is_app_origin(scheme: &str, host: Option<&str>, port: Option<u16>) -> bool {
    port.is_none()
        && host.is_some_and(|host| {
            APP_ORIGINS
                .iter()
                .any(|(s, h)| scheme.eq_ignore_ascii_case(s) && host.eq_ignore_ascii_case(h))
        })
}

#[must_use]
pub fn navigation(target: &str) -> Navigation {
    if IN_APP_BLANK.contains(&target) {
        return Navigation::InApp;
    }
    match url::Url::parse(target) {
        Ok(url) if is_app_origin(url.scheme(), url.host_str(), url.port()) => Navigation::InApp,
        _ => Navigation::External,
    }
}

#[must_use]
pub fn ipc_origin_allowed(source: &http::Uri) -> bool {
    source
        .scheme_str()
        .is_some_and(|scheme| is_app_origin(scheme, source.host(), source.port_u16()))
}
