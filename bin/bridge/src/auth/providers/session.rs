use crate::auth::loopback::{LOOPBACK_TIMEOUT_SECS, LoopbackServer};
use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::auth::types::{HelperOutput, SessionExchangeRequest};
use crate::config::Config;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;
use async_trait::async_trait;
use std::process::Command;
use std::time::Duration;
use systemprompt_identifiers::{SessionId, ValidatedUrl};

pub struct SessionProvider {
    base_url: ValidatedUrl,
    configured: bool,
}

impl SessionProvider {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let configured = config
            .session
            .as_ref()
            .is_some_and(|s| s.enabled.unwrap_or(true));
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            configured,
        }
    }
}

#[async_trait]
impl AuthProvider for SessionProvider {
    fn name(&self) -> &'static str {
        "session"
    }

    async fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        if !self.configured {
            return Err(AuthError::NotConfigured);
        }

        let server = LoopbackServer::bind()
            .await
            .map_err(|e| AuthError::Failed {
                provider: "session",
                source: AuthFailedSource::Loopback(e),
            })?;
        let callback = server.callback_url();
        let auth_url = build_auth_url(self.base_url.as_str(), callback.as_str());

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

        let req = SessionExchangeRequest {
            code: captured.code,
            session_id: SessionId::generate(),
        };
        let client = GatewayClient::new(self.base_url.clone());
        let resp = client
            .session_exchange(&req)
            .await
            .map_err(|e| AuthError::Failed {
                provider: "session",
                source: AuthFailedSource::Gateway(e),
            })?;
        Ok(resp.into())
    }
}

fn build_auth_url(base: &str, callback: &str) -> String {
    let encoded = encode_component(callback);
    format!(
        "{}/cowork/device-link?redirect={encoded}",
        base.trim_end_matches('/')
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

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + nibble - 10) as char,
        _ => '0',
    }
}

fn launch_browser(url: &str) -> std::io::Result<()> {
    let (program, args) = browser_command(url);
    Command::new(program).args(args).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
fn browser_command(url: &str) -> (&'static str, Vec<String>) {
    ("open", vec![url.to_string()])
}

#[cfg(target_os = "windows")]
fn browser_command(url: &str) -> (&'static str, Vec<String>) {
    (
        "cmd",
        vec!["/C".into(), "start".into(), String::new(), url.into()],
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn browser_command(url: &str) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![url.to_string()])
}
