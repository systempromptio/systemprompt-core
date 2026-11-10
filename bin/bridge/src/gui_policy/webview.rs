//! Which pages the settings webview may show in-window and which may speak
//! to the bridge over IPC.
//!
//! The app has exactly one origin per platform: the one its custom-protocol
//! handler serves. On macOS and Linux wry registers the `sp` scheme itself,
//! so the app is `sp://app`. On Windows (`WebView2`) wry cannot register a
//! scheme and instead intercepts `https://sp.*` resource requests
//! (`with_https_scheme(true)`), so the app is `https://sp.app`. `.app` is a
//! public TLD, so `https://sp.app` is a network origin everywhere the
//! handler does not intercept it, and is never the app there.
//!
//! Both decisions are made on a parsed URL: the scheme and host must equal
//! the platform's origin exactly, with no port or userinfo, so a look-alike
//! host such as `sp.app.example.com` is never treated as the app. These
//! functions carry no platform dependency, so both origins are compiled and
//! tested on every target even though the webview itself is desktop-only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const IN_APP_BLANK: [&str; 2] = ["about:blank", "about:srcdoc"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    InApp,
    External,
}

/// The single origin a platform's custom-protocol handler serves the app
/// from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppOrigin {
    scheme: &'static str,
    host: &'static str,
}

impl AppOrigin {
    pub const CUSTOM_SCHEME: Self = Self {
        scheme: "sp",
        host: "app",
    };
    pub const WEBVIEW2_HTTPS: Self = Self {
        scheme: "https",
        host: "sp.app",
    };

    #[must_use]
    pub const fn current() -> Self {
        if cfg!(any(target_os = "windows", target_os = "android")) {
            Self::WEBVIEW2_HTTPS
        } else {
            Self::CUSTOM_SCHEME
        }
    }

    #[must_use]
    pub fn index_url(self) -> String {
        format!("{}://{}/index.html", self.scheme, self.host)
    }

    fn matches(self, scheme: &str, host: Option<&str>, port: Option<u16>) -> bool {
        port.is_none()
            && scheme.eq_ignore_ascii_case(self.scheme)
            && host.is_some_and(|host| host.eq_ignore_ascii_case(self.host))
    }

    #[must_use]
    pub fn navigation(self, target: &str) -> Navigation {
        if IN_APP_BLANK.contains(&target) {
            return Navigation::InApp;
        }
        match url::Url::parse(target) {
            Ok(url)
                if url.username().is_empty()
                    && url.password().is_none()
                    && self.matches(url.scheme(), url.host_str(), url.port()) =>
            {
                Navigation::InApp
            },
            _ => Navigation::External,
        }
    }

    #[must_use]
    pub fn admits_ipc(self, source: &http::Uri) -> bool {
        let no_userinfo = source
            .authority()
            .is_some_and(|authority| !authority.as_str().contains('@'));
        no_userinfo
            && source
                .scheme_str()
                .is_some_and(|scheme| self.matches(scheme, source.host(), source.port_u16()))
    }
}
