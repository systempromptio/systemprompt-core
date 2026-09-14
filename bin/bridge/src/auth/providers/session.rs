//! Auth provider reusing an existing browser session grant.
//!
//! The provider itself never opens a browser: it is only ever run from the
//! background token cache, where a consent pop-up would appear unbidden on
//! every refresh tick. When the cached session token is gone it reports
//! `SignInRequired` and the GUI asks the user. The interactive device-link
//! capture (`capture_device_link_code`) lives here for the GUI sign-in
//! handler, which is the one place a browser may be launched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::auth::loopback::{LOOPBACK_TIMEOUT_SECS, LoopbackServer};
use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::config::Config;
use crate::gateway::types::HelperOutput;
use crate::stdio::diag;
use async_trait::async_trait;
use std::time::Duration;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

#[derive(Debug, Clone, Copy)]
pub struct SessionProvider {
    configured: bool,
}

impl SessionProvider {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let configured = config
            .session
            .as_ref()
            .is_some_and(|s| s.enabled.unwrap_or(false));
        Self { configured }
    }
}

#[async_trait]
impl AuthProvider for SessionProvider {
    fn name(&self) -> &'static str {
        "session"
    }

    async fn authenticate(
        &self,
        _session_id: &SessionId,
        _http: &reqwest::Client,
    ) -> Result<HelperOutput, AuthError> {
        if !self.configured {
            return Err(AuthError::NotConfigured);
        }
        Err(AuthError::Failed {
            provider: "session",
            source: AuthFailedSource::SignInRequired,
        })
    }
}

pub async fn capture_device_link_code(base_url: &ValidatedUrl) -> Result<String, AuthError> {
    let server = LoopbackServer::bind().await.map_err(|e| {
        diag(&format!("loopback callback listener unavailable: {e}"));
        AuthError::Failed {
            provider: "session",
            source: AuthFailedSource::Loopback(e),
        }
    })?;
    capture_on(server, base_url).await
}

pub async fn capture_on(
    server: LoopbackServer,
    base_url: &ValidatedUrl,
) -> Result<String, AuthError> {
    let callback = server.callback_url();
    let auth_url = build_auth_url(base_url.as_str(), Some(callback.as_str()));

    diag(&format!("opening browser to {auth_url}"));
    if let Err(e) = launch_browser(&auth_url) {
        diag(&format!("could not launch browser automatically: {e}"));
        diag(&format!("open manually: {auth_url}"));
    }

    let captured = server
        .accept_callback(Duration::from_secs(LOOPBACK_TIMEOUT_SECS))
        .await
        .map_err(|e| AuthError::Failed {
            provider: "session",
            source: AuthFailedSource::Loopback(e),
        })?;
    Ok(captured.code)
}

#[must_use]
pub fn device_link_url(base: &str, callback: Option<&str>) -> String {
    build_auth_url(base, callback)
}

fn build_auth_url(base: &str, callback: Option<&str>) -> String {
    let path = crate::brand::brand().device_link_path;
    let base = base.trim_end_matches('/');
    callback.map_or_else(
        || format!("{base}{path}"),
        |callback| {
            let encoded = encode_component(callback);
            format!("{base}{path}?redirect={encoded}")
        },
    )
}

fn encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            },
            _ => {
                out.push('%');
                out.push(hex_upper(byte >> 4));
                out.push(hex_upper(byte & 0x0f));
            },
        }
    }
    out
}

const fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + nibble - 10) as char,
        _ => '0',
    }
}

// Why: the URL is handed to the platform opener (ShellExecute, `open`,
// `xdg-open`) as one argument; it never passes through a shell where `&`
// or `|` in a query string would start a second command.
fn launch_browser(url: &str) -> std::io::Result<()> {
    opener::open(url).map_err(std::io::Error::other)
}
