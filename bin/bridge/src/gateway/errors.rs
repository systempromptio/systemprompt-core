//! Gateway client error taxonomy.

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
    #[error("gateway rejected hook token request: status={status} body={body}")]
    HookTokenRejected {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("gateway request failed: {0}")]
    PostRequest(Box<reqwest::Error>),
    #[error("malformed gateway response: {0}")]
    AuthDecode(Box<reqwest::Error>),
    #[error("gateway returned status {status} from {endpoint}")]
    HttpStatus {
        status: reqwest::StatusCode,
        endpoint: &'static str,
    },
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
}
