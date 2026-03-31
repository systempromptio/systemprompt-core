pub mod api;
pub mod dynamic_registration;

use std::fmt;
use std::str::FromStr;

pub use systemprompt_models::auth::JwtClaims;
pub use systemprompt_models::oauth::OAuthServerConfig as OAuthConfig;

macro_rules! impl_str_enum {
    ($enum_name:ident, $error_variant:ident, { $($variant:ident => $str:expr),+ $(,)? }) => {
        impl $enum_name {
            pub const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $str),+
                }
            }
        }

        impl FromStr for $enum_name {
            type Err = OAuthParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($str => Ok(Self::$variant)),+,
                    _ => Err(OAuthParseError::$error_variant(s.to_string())),
                }
            }
        }

        impl fmt::Display for $enum_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum OAuthParseError {
    InvalidGrantType(String),
    InvalidPkceMethod(String),
    InvalidResponseType(String),
    InvalidResponseMode(String),
    InvalidDisplayMode(String),
    InvalidPrompt(String),
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

impl_str_enum!(GrantType, InvalidGrantType, {
    AuthorizationCode => "authorization_code",
    RefreshToken => "refresh_token",
    ClientCredentials => "client_credentials",
});

impl GrantType {
    pub const fn default_grant_types() -> &'static [&'static str] {
        &["authorization_code", "refresh_token"]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    S256,
    Plain,
}

impl_str_enum!(PkceMethod, InvalidPkceMethod, {
    S256 => "S256",
    Plain => "plain",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    Code,
}

impl_str_enum!(ResponseType, InvalidResponseType, {
    Code => "code",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Query,
    Fragment,
}

impl_str_enum!(ResponseMode, InvalidResponseMode, {
    Query => "query",
    Fragment => "fragment",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Page,
    Popup,
    Touch,
    Wap,
}

impl_str_enum!(DisplayMode, InvalidDisplayMode, {
    Page => "page",
    Popup => "popup",
    Touch => "touch",
    Wap => "wap",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    None,
    Login,
    Consent,
    SelectAccount,
}

impl_str_enum!(Prompt, InvalidPrompt, {
    None => "none",
    Login => "login",
    Consent => "consent",
    SelectAccount => "select_account",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenAuthMethod {
    ClientSecretPost,
    ClientSecretBasic,
    None,
}

impl_str_enum!(TokenAuthMethod, InvalidTokenAuthMethod, {
    ClientSecretPost => "client_secret_post",
    ClientSecretBasic => "client_secret_basic",
    None => "none",
});

impl TokenAuthMethod {
    pub const fn default() -> Self {
        Self::ClientSecretPost
    }
}
