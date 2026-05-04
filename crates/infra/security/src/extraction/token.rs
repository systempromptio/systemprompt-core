use axum::http::HeaderMap;
use std::error::Error;
use std::fmt;

const DEFAULT_COOKIE_NAME: &str = "access_token";
const DEFAULT_MCP_HEADER_NAME: &str = "x-mcp-proxy-auth";
const BEARER_PREFIX: &str = "Bearer ";

/// Source from which a [`TokenExtractor`] should attempt to read the
/// bearer token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionMethod {
    /// `Authorization: Bearer …` header.
    AuthorizationHeader,
    /// `x-mcp-proxy-auth: Bearer …` header (MCP proxy contract).
    McpProxyHeader,
    /// Browser session cookie.
    Cookie,
}

impl fmt::Display for ExtractionMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationHeader => write!(f, "Authorization header"),
            Self::McpProxyHeader => write!(f, "MCP proxy header"),
            Self::Cookie => write!(f, "Cookie"),
        }
    }
}

/// Bearer-token extractor with a configurable fallback chain.
///
/// Tries each [`ExtractionMethod`] in order and returns the first token
/// that parses successfully. The standard chain (header → MCP proxy →
/// cookie) handles every transport contract the API supports.
#[derive(Debug, Clone)]
pub struct TokenExtractor {
    fallback_chain: Vec<ExtractionMethod>,
    cookie_name: String,
    mcp_header_name: String,
}

impl TokenExtractor {
    /// Constructs an extractor with an explicit fallback chain.
    #[must_use]
    pub fn new(fallback_chain: Vec<ExtractionMethod>) -> Self {
        Self {
            fallback_chain,
            cookie_name: DEFAULT_COOKIE_NAME.to_string(),
            mcp_header_name: DEFAULT_MCP_HEADER_NAME.to_string(),
        }
    }

    /// Overrides the cookie name used by the [`ExtractionMethod::Cookie`]
    /// step.
    #[must_use]
    pub fn with_cookie_name(mut self, name: String) -> Self {
        self.cookie_name = name;
        self
    }

    /// Overrides the MCP proxy header name used by the
    /// [`ExtractionMethod::McpProxyHeader`] step.
    #[must_use]
    pub fn with_mcp_header_name(mut self, name: String) -> Self {
        self.mcp_header_name = name;
        self
    }

    /// Returns the standard fallback chain: header → MCP proxy → cookie.
    #[must_use]
    pub fn standard() -> Self {
        Self::new(vec![
            ExtractionMethod::AuthorizationHeader,
            ExtractionMethod::McpProxyHeader,
            ExtractionMethod::Cookie,
        ])
    }

    /// Returns a chain suited to browser-only flows: header → cookie.
    #[must_use]
    pub fn browser_only() -> Self {
        Self::new(vec![
            ExtractionMethod::AuthorizationHeader,
            ExtractionMethod::Cookie,
        ])
    }

    /// Returns a chain that only accepts the bearer header.
    #[must_use]
    pub fn api_only() -> Self {
        Self::new(vec![ExtractionMethod::AuthorizationHeader])
    }

    /// Returns the configured fallback chain.
    #[must_use]
    pub fn chain(&self) -> &[ExtractionMethod] {
        &self.fallback_chain
    }

    /// Walks the fallback chain and returns the first successfully
    /// extracted token.
    ///
    /// # Errors
    ///
    /// Returns [`TokenExtractionError::NoTokenFound`] when every method
    /// in the chain fails.
    pub fn extract(&self, headers: &HeaderMap) -> Result<String, TokenExtractionError> {
        for method in &self.fallback_chain {
            match method {
                ExtractionMethod::AuthorizationHeader => {
                    if let Ok(token) = Self::extract_from_authorization(headers) {
                        return Ok(token);
                    }
                },
                ExtractionMethod::McpProxyHeader => {
                    if let Ok(token) = self.extract_from_mcp_proxy(headers) {
                        return Ok(token);
                    }
                },
                ExtractionMethod::Cookie => {
                    if let Ok(token) = self.extract_from_cookie(headers) {
                        return Ok(token);
                    }
                },
            }
        }

        Err(TokenExtractionError::NoTokenFound)
    }

    /// Extracts the bearer token from the `Authorization` header.
    ///
    /// # Errors
    ///
    /// Returns a [`TokenExtractionError`] variant describing whether the
    /// header was missing or malformed.
    pub fn extract_from_authorization(headers: &HeaderMap) -> Result<String, TokenExtractionError> {
        let auth_headers = headers.get_all("authorization");

        if auth_headers.iter().count() == 0 {
            return Err(TokenExtractionError::MissingAuthorizationHeader);
        }

        for auth_value in &auth_headers {
            let Ok(auth_header) = auth_value.to_str().map_err(|e| {
                tracing::debug!(error = %e, "Authorization header contains non-ASCII characters");
                e
            }) else {
                continue;
            };

            if let Some(token) = auth_header.strip_prefix(BEARER_PREFIX) {
                let token = token.trim();
                if !token.is_empty() {
                    return Ok(token.to_string());
                }
            }
        }

        Err(TokenExtractionError::InvalidAuthorizationFormat)
    }

    /// Extracts the bearer token from the configured MCP proxy header.
    ///
    /// # Errors
    ///
    /// Same shape as [`Self::extract_from_authorization`].
    pub fn extract_from_mcp_proxy(
        &self,
        headers: &HeaderMap,
    ) -> Result<String, TokenExtractionError> {
        let header_value = headers
            .get(&self.mcp_header_name)
            .ok_or(TokenExtractionError::MissingMcpProxyHeader)?;

        let auth_header = header_value
            .to_str()
            .map_err(|_| TokenExtractionError::InvalidMcpProxyFormat)?;

        auth_header
            .strip_prefix(BEARER_PREFIX)
            .ok_or(TokenExtractionError::InvalidMcpProxyFormat)
            .map(ToString::to_string)
    }

    /// Extracts the bearer token from the configured session cookie.
    ///
    /// # Errors
    ///
    /// Same shape as [`Self::extract_from_authorization`].
    pub fn extract_from_cookie(&self, headers: &HeaderMap) -> Result<String, TokenExtractionError> {
        let cookie_header = headers
            .get("cookie")
            .ok_or(TokenExtractionError::MissingCookie)?
            .to_str()
            .map_err(|_| TokenExtractionError::InvalidCookieFormat)?;

        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            let cookie_prefix = format!("{}=", self.cookie_name);
            if let Some(value) = cookie.strip_prefix(&cookie_prefix) {
                if !value.is_empty() {
                    return Ok(value.to_string());
                }
            }
        }

        Err(TokenExtractionError::TokenNotFoundInCookie)
    }
}

/// Failures produced while extracting a bearer token from a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenExtractionError {
    /// Every method in the fallback chain failed.
    NoTokenFound,
    /// The `Authorization` header is missing.
    MissingAuthorizationHeader,
    /// The `Authorization` header is malformed (not `Bearer <token>`).
    InvalidAuthorizationFormat,
    /// The MCP proxy header is missing.
    MissingMcpProxyHeader,
    /// The MCP proxy header is malformed.
    InvalidMcpProxyFormat,
    /// The `Cookie` header is missing.
    MissingCookie,
    /// The `Cookie` header is malformed.
    InvalidCookieFormat,
    /// The configured cookie name was not present in the `Cookie` header.
    TokenNotFoundInCookie,
}

impl fmt::Display for TokenExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTokenFound => write!(f, "No token found in request"),
            Self::MissingAuthorizationHeader => {
                write!(f, "Missing Authorization header")
            },
            Self::InvalidAuthorizationFormat => {
                write!(
                    f,
                    "Invalid Authorization header format (expected 'Bearer <token>')"
                )
            },
            Self::MissingMcpProxyHeader => {
                write!(f, "Missing MCP proxy authorization header")
            },
            Self::InvalidMcpProxyFormat => {
                write!(
                    f,
                    "Invalid MCP proxy header format (expected 'Bearer <token>')"
                )
            },
            Self::MissingCookie => write!(f, "Missing cookie header"),
            Self::InvalidCookieFormat => write!(f, "Invalid cookie format"),
            Self::TokenNotFoundInCookie => write!(f, "Token not found in cookies"),
        }
    }
}

impl Error for TokenExtractionError {}
