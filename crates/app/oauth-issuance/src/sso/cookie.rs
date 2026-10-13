//! The binder cookie that ties an SSO return to the browser that started it.
//!
//! It is scoped to the provider's callback path so no other request carries
//! it, and it lives exactly as long as the state row it binds. `Secure` is
//! dropped only when the API is served over plain `http`, where a browser
//! would refuse to store it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::sso::CookieSameSite;

pub const BINDER_COOKIE_NAME: &str = "sp_sso_bind";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSpec {
    pub name: &'static str,
    pub value: String,
    pub path: String,
    pub max_age: i64,
    pub same_site: CookieSameSite,
    pub secure: bool,
}

impl CookieSpec {
    #[must_use]
    pub fn binder(
        value: String,
        path: &str,
        max_age: i64,
        same_site: CookieSameSite,
        api_external_url: &str,
    ) -> Self {
        let secure = !api_external_url.starts_with("http://");
        let same_site = if secure {
            same_site
        } else {
            CookieSameSite::Lax
        };
        Self {
            name: BINDER_COOKIE_NAME,
            value,
            path: path.to_owned(),
            max_age,
            same_site,
            secure,
        }
    }

    #[must_use]
    pub fn header_value(&self) -> String {
        let secure = if self.secure { "; Secure" } else { "" };
        format!(
            "{}={}; Path={}; HttpOnly; SameSite={}; Max-Age={}{secure}",
            self.name,
            self.value,
            self.path,
            self.same_site.as_str(),
            self.max_age,
        )
    }

    #[must_use]
    pub fn clear(&self) -> Self {
        Self {
            value: String::new(),
            max_age: 0,
            ..self.clone()
        }
    }
}

#[must_use]
pub fn binder_from_cookie_header(header: &str) -> Option<&str> {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == BINDER_COOKIE_NAME)
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
}
