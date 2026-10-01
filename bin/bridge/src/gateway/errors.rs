//! Gateway client error taxonomy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("pubkey fetch failed: {0}")]
    PubkeyFetch(Box<reqwest::Error>),
    #[error("malformed pubkey response: {0}")]
    PubkeyDecode(Box<reqwest::Error>),
    #[error("pubkey field missing in response")]
    PubkeyMissing,
    #[error("manifest fetch failed: {0}")]
    ManifestFetch(Box<reqwest::Error>),
    #[error("malformed manifest response: {0}")]
    ManifestDecode(Box<reqwest::Error>),
    #[error(
        "manifest response is not a signed envelope this bridge understands ({source}); response \
         starts with: {snippet}"
    )]
    ManifestEnvelopeShape {
        snippet: String,
        source: serde_json::Error,
    },
    #[error("refused unsafe path: {0}")]
    UnsafePath(String),
    #[error("plugin fetch {plugin_id}:{path} failed: {source}")]
    PluginFetch {
        plugin_id: String,
        path: String,
        source: Box<reqwest::Error>,
    },
    #[error("plugin read {plugin_id}:{path} failed: {source}")]
    PluginRead {
        plugin_id: String,
        path: String,
        source: Box<reqwest::Error>,
    },
    #[error("whoami fetch failed: {0}")]
    WhoamiFetch(Box<reqwest::Error>),
    #[error("malformed whoami response: {0}")]
    WhoamiDecode(Box<reqwest::Error>),
    #[error("health check failed: {0}")]
    HealthCheck(Box<reqwest::Error>),
    #[error("bridge profile fetch failed: {0}")]
    ProfileFetch(Box<reqwest::Error>),
    #[error("malformed bridge profile response: {0}")]
    ProfileDecode(Box<reqwest::Error>),
    #[error("bridge profile usage fetch failed: {0}")]
    ProfileUsageFetch(Box<reqwest::Error>),
    #[error("malformed bridge profile usage response: {0}")]
    ProfileUsageDecode(Box<reqwest::Error>),
    #[error("gateway PAT request failed: {0}")]
    PatRequest(Box<reqwest::Error>),
    #[error("gateway oauth-client provisioning failed: {0}")]
    OAuthClientRequest(Box<reqwest::Error>),
    #[error("malformed oauth-client response: {0}")]
    OAuthClientDecode(Box<reqwest::Error>),
    #[error("plugin hook token request failed: {0}")]
    HookTokenRequest(Box<reqwest::Error>),
    #[error("malformed hook token response: {0}")]
    HookTokenDecode(Box<reqwest::Error>),
    #[error("gateway request failed: {0}")]
    PostRequest(Box<reqwest::Error>),
    #[error("malformed gateway response: {0}")]
    AuthDecode(Box<reqwest::Error>),
    #[error("gateway returned status {status} from {endpoint}: {rejection}")]
    Rejected {
        endpoint: &'static str,
        status: reqwest::StatusCode,
        rejection: Box<GatewayRejection>,
    },
    #[error("release manifest fetch failed: {0}")]
    ReleaseFetch(Box<reqwest::Error>),
    #[error("malformed release manifest response: {0}")]
    ReleaseDecode(Box<reqwest::Error>),
    #[error("malformed device enrolment response: {0}")]
    DeviceEnrollDecode(Box<reqwest::Error>),
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
}

const REJECTION_EXCERPT_CHARS: usize = 240;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GatewayRejection {
    pub code: Option<String>,
    pub error_key: Option<String>,
    pub message: Option<String>,
    pub excerpt: String,
}

// JSON: gateway or OAuth error body — `code` and `error` arrive as a string or
// a number depending on which server produced the rejection.
#[derive(serde::Deserialize)]
struct RejectionBody {
    // JSON: `code` is a string from the gateway, a number from some proxies.
    #[serde(default)]
    code: Option<serde_json::Value>,
    #[serde(default)]
    error_key: Option<String>,
    #[serde(default)]
    message: Option<String>,
    // JSON: OAuth `error` code, string or number.
    #[serde(default)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    error_description: Option<String>,
}

// JSON: a rejection code field that may be a string or a number.
fn json_text(value: Option<serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

impl GatewayRejection {
    #[must_use]
    pub fn parse(body: &str) -> Self {
        let trimmed = body.trim();
        let excerpt = trimmed.chars().take(REJECTION_EXCERPT_CHARS).collect();
        let Ok(parsed) = serde_json::from_str::<RejectionBody>(trimmed) else {
            return Self {
                excerpt,
                ..Self::default()
            };
        };
        Self {
            code: json_text(parsed.code).or_else(|| json_text(parsed.error)),
            error_key: parsed.error_key,
            message: parsed.message.or(parsed.error_description),
            excerpt,
        }
    }

    pub async fn read(resp: reqwest::Response) -> (reqwest::StatusCode, Box<Self>) {
        let status = resp.status();
        let body = match resp.text().await {
            Ok(body) => body,
            Err(e) => {
                tracing::warn!(error = %e, %status, "gateway rejection body unreadable");
                String::new()
            },
        };
        (status, Box::new(Self::parse(&body)))
    }
}

impl std::fmt::Display for GatewayRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.message, self.excerpt.is_empty()) {
            (Some(message), _) => f.write_str(message),
            (None, false) => f.write_str(&self.excerpt),
            (None, true) => f.write_str("no response body"),
        }
    }
}
