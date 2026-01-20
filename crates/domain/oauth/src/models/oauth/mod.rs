pub mod api;
pub mod dynamic_registration;

use std::fmt;
use std::str::FromStr;

pub use systemprompt_models::auth::JwtClaims;
pub use systemprompt_models::oauth::OAuthServerConfig as OAuthConfig;

/// Error type for OAuth-related parsing failures.
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum OAuthParseError {
    /// Unknown grant type.
    InvalidGrantType(String),
    /// Unknown PKCE method.
    InvalidPkceMethod(String),
    /// Unknown response type.
    InvalidResponseType(String),
    /// Unknown response mode.
    InvalidResponseMode(String),
    /// Unknown display mode.
    InvalidDisplayMode(String),
    /// Unknown prompt type.
    InvalidPrompt(String),
    /// Unknown token auth method.
    InvalidTokenAuthMethod(String),
}

impl fmt::Display for OAuthParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrantType(s) => write!(f, "unknown grant type: '{}'", s),
            Self::InvalidPkceMethod(s) => write!(f, "unknown PKCE method: '{}'", s),
            Self::InvalidResponseType(s) => write!(f, "unknown response type: '{}'", s),
            Self::InvalidResponseMode(s) => write!(f, "unknown response mode: '{}'", s),
            Self::InvalidDisplayMode(s) => write!(f, "unknown display mode: '{}'", s),
            Self::InvalidPrompt(s) => write!(f, "unknown prompt type: '{}'", s),
            Self::InvalidTokenAuthMethod(s) => write!(f, "unknown token auth method: '{}'", s),
        }
    }
}

impl std::error::Error for OAuthParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantType {
    AuthorizationCode,
    RefreshToken,
    ClientCredentials,
}

impl GrantType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AuthorizationCode => "authorization_code",
            Self::RefreshToken => "refresh_token",
            Self::ClientCredentials => "client_credentials",
        }
    }

    pub const fn default_grant_types() -> &'static [&'static str] {
        &["authorization_code", "refresh_token"]
    }
}

impl FromStr for GrantType {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "authorization_code" => Ok(Self::AuthorizationCode),
            "refresh_token" => Ok(Self::RefreshToken),
            "client_credentials" => Ok(Self::ClientCredentials),
            _ => Err(OAuthParseError::InvalidGrantType(s.to_string())),
        }
    }
}

impl fmt::Display for GrantType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    S256,
    Plain,
}

impl PkceMethod {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::S256 => "S256",
            Self::Plain => "plain",
        }
    }
}

impl FromStr for PkceMethod {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "S256" => Ok(Self::S256),
            "plain" => Ok(Self::Plain),
            _ => Err(OAuthParseError::InvalidPkceMethod(s.to_string())),
        }
    }
}

impl fmt::Display for PkceMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    Code,
}

impl ResponseType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
        }
    }
}

impl FromStr for ResponseType {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "code" => Ok(Self::Code),
            _ => Err(OAuthParseError::InvalidResponseType(s.to_string())),
        }
    }
}

impl fmt::Display for ResponseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Query,
    Fragment,
}

impl ResponseMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Fragment => "fragment",
        }
    }
}

impl FromStr for ResponseMode {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "query" => Ok(Self::Query),
            "fragment" => Ok(Self::Fragment),
            _ => Err(OAuthParseError::InvalidResponseMode(s.to_string())),
        }
    }
}

impl fmt::Display for ResponseMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Page,
    Popup,
    Touch,
    Wap,
}

impl DisplayMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Popup => "popup",
            Self::Touch => "touch",
            Self::Wap => "wap",
        }
    }
}

impl FromStr for DisplayMode {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "page" => Ok(Self::Page),
            "popup" => Ok(Self::Popup),
            "touch" => Ok(Self::Touch),
            "wap" => Ok(Self::Wap),
            _ => Err(OAuthParseError::InvalidDisplayMode(s.to_string())),
        }
    }
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    None,
    Login,
    Consent,
    SelectAccount,
}

impl Prompt {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Login => "login",
            Self::Consent => "consent",
            Self::SelectAccount => "select_account",
        }
    }
}

impl FromStr for Prompt {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "login" => Ok(Self::Login),
            "consent" => Ok(Self::Consent),
            "select_account" => Ok(Self::SelectAccount),
            _ => Err(OAuthParseError::InvalidPrompt(s.to_string())),
        }
    }
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenAuthMethod {
    ClientSecretPost,
    ClientSecretBasic,
    None,
}

impl TokenAuthMethod {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ClientSecretPost => "client_secret_post",
            Self::ClientSecretBasic => "client_secret_basic",
            Self::None => "none",
        }
    }

    pub const fn default() -> Self {
        Self::ClientSecretPost
    }
}

impl FromStr for TokenAuthMethod {
    type Err = OAuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "client_secret_post" => Ok(Self::ClientSecretPost),
            "client_secret_basic" => Ok(Self::ClientSecretBasic),
            "none" => Ok(Self::None),
            _ => Err(OAuthParseError::InvalidTokenAuthMethod(s.to_string())),
        }
    }
}

impl fmt::Display for TokenAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
