//! Core OAuth model types: grant types, response modes, JWT claims, dynamic
//! registration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod api;
pub mod dynamic_registration;

use std::fmt;
use std::str::FromStr;

pub use systemprompt_models::auth::JwtClaims;

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
                    _ => Err(OAuthParseError::$error_variant {
                        value: s.to_owned(),
                    }),
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
pub enum OAuthParseError {
    GrantType { value: String },
    PkceMethod { value: String },
    ResponseType { value: String },
    ResponseMode { value: String },
    DisplayMode { value: String },
    Prompt { value: String },
    TokenAuthMethod { value: String },
}

impl fmt::Display for OAuthParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GrantType { value } => write!(f, "unknown grant type: '{value}'"),
            Self::PkceMethod { value } => write!(f, "unknown PKCE method: '{value}'"),
            Self::ResponseType { value } => write!(f, "unknown response type: '{value}'"),
            Self::ResponseMode { value } => write!(f, "unknown response mode: '{value}'"),
            Self::DisplayMode { value } => write!(f, "unknown display mode: '{value}'"),
            Self::Prompt { value } => write!(f, "unknown prompt type: '{value}'"),
            Self::TokenAuthMethod { value } => write!(f, "unknown token auth method: '{value}'"),
        }
    }
}

impl std::error::Error for OAuthParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantType {
    AuthorizationCode,
    RefreshToken,
    ClientCredentials,
    TokenExchange,
    JwtBearer,
}

impl_str_enum!(GrantType, GrantType, {
    AuthorizationCode => "authorization_code",
    RefreshToken => "refresh_token",
    ClientCredentials => "client_credentials",
    TokenExchange => "urn:ietf:params:oauth:grant-type:token-exchange",
    JwtBearer => "urn:ietf:params:oauth:grant-type:jwt-bearer",
});

impl GrantType {
    pub const fn default_grant_types() -> &'static [&'static str] {
        &["authorization_code", "refresh_token"]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    S256,
}

impl_str_enum!(PkceMethod, PkceMethod, {
    S256 => "S256",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    Code,
}

impl_str_enum!(ResponseType, ResponseType, {
    Code => "code",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Query,
    Fragment,
}

impl_str_enum!(ResponseMode, ResponseMode, {
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

impl_str_enum!(DisplayMode, DisplayMode, {
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

impl_str_enum!(Prompt, Prompt, {
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

impl_str_enum!(TokenAuthMethod, TokenAuthMethod, {
    ClientSecretPost => "client_secret_post",
    ClientSecretBasic => "client_secret_basic",
    None => "none",
});

impl TokenAuthMethod {
    pub const fn default() -> Self {
        Self::ClientSecretPost
    }
}
