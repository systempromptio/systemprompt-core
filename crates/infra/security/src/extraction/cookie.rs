use axum::http::HeaderMap;

/// Extracts the access token from the browser session cookie.
///
/// Defaults to the `access_token` cookie name; callers can override via
/// [`Self::new`] when the deployment uses a custom cookie.
#[derive(Debug, Clone)]
pub struct CookieExtractor {
    cookie_name: String,
}

impl Default for CookieExtractor {
    fn default() -> Self {
        Self::new(Self::DEFAULT_COOKIE_NAME)
    }
}

impl CookieExtractor {
    /// Default cookie name used when the deployment has not overridden it.
    pub const DEFAULT_COOKIE_NAME: &'static str = "access_token";

    /// Constructs a new extractor that reads the cookie named
    /// `cookie_name`.
    pub fn new(cookie_name: impl Into<String>) -> Self {
        Self {
            cookie_name: cookie_name.into(),
        }
    }

    /// Extracts the token value from the configured cookie.
    ///
    /// # Errors
    ///
    /// Returns a [`CookieExtractionError`] variant describing whether the
    /// `Cookie` header was missing, malformed, or did not contain the
    /// expected name.
    pub fn extract(&self, headers: &HeaderMap) -> Result<String, CookieExtractionError> {
        self.extract_internal(headers)
    }

    /// Convenience constructor + extract using the default `access_token`
    /// cookie name.
    ///
    /// # Errors
    ///
    /// Same as [`Self::extract`].
    pub fn extract_access_token(headers: &HeaderMap) -> Result<String, CookieExtractionError> {
        Self::default().extract(headers)
    }

    fn extract_internal(&self, headers: &HeaderMap) -> Result<String, CookieExtractionError> {
        let cookie_header = headers
            .get("cookie")
            .ok_or(CookieExtractionError::MissingCookie)?
            .to_str()
            .map_err(|_| CookieExtractionError::InvalidCookieFormat)?;

        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            let cookie_prefix = format!("{}=", self.cookie_name);
            if let Some(value) = cookie.strip_prefix(&cookie_prefix) {
                if !value.is_empty() {
                    return Ok(value.to_string());
                }
            }
        }

        Err(CookieExtractionError::TokenNotFoundInCookie)
    }
}

/// Failures produced while extracting a bearer token from a cookie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieExtractionError {
    /// The request did not carry a `Cookie` header.
    MissingCookie,
    /// The `Cookie` header value was not a valid ASCII string.
    InvalidCookieFormat,
    /// The configured cookie name was not present in the header.
    TokenNotFoundInCookie,
}

impl std::fmt::Display for CookieExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCookie => write!(f, "Missing cookie header"),
            Self::InvalidCookieFormat => write!(f, "Invalid cookie format"),
            Self::TokenNotFoundInCookie => {
                write!(f, "Access token not found in cookies")
            },
        }
    }
}

impl std::error::Error for CookieExtractionError {}
