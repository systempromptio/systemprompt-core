pub mod api;
pub mod dynamic_registration;

use std::fmt;
use std::str::FromStr;

pub use systemprompt_models::auth::JwtClaims;
pub use systemprompt_models::oauth::OAuthServerConfig as OAuthConfig;

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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "authorization_code" => Ok(Self::AuthorizationCode),
            "refresh_token" => Ok(Self::RefreshToken),
            "client_credentials" => Ok(Self::ClientCredentials),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "S256" => Ok(Self::S256),
            "plain" => Ok(Self::Plain),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "code" => Ok(Self::Code),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "query" => Ok(Self::Query),
            "fragment" => Ok(Self::Fragment),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "page" => Ok(Self::Page),
            "popup" => Ok(Self::Popup),
            "touch" => Ok(Self::Touch),
            "wap" => Ok(Self::Wap),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "login" => Ok(Self::Login),
            "consent" => Ok(Self::Consent),
            "select_account" => Ok(Self::SelectAccount),
            _ => Err(()),
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "client_secret_post" => Ok(Self::ClientSecretPost),
            "client_secret_basic" => Ok(Self::ClientSecretBasic),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

impl fmt::Display for TokenAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
