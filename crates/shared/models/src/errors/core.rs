//! The legacy [`CoreError`] umbrella enum — a closed set of variants
//! covering authentication, session, module, and IO failure modes with
//! HTTP status code mapping helpers.

/// Umbrella error type carrying both 4xx and 5xx flavours.
///
/// New code should prefer one of the focused enums in sibling modules
/// (e.g. [`super::ConfigValidationError`], [`super::SecretsError`])
/// and only convert into [`CoreError`] at the API boundary.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreError {
    /// A module manifest was missing a required field.
    #[error("Module config missing required field: {field}")]
    MissingConfigField {
        /// Name of the missing field.
        field: String,
    },

    /// A version string was not a valid semver value.
    #[error("Invalid module version: {version}")]
    InvalidVersion {
        /// The offending version string.
        version: String,
    },

    /// A module manifest violated a structural constraint.
    #[error("Module {name} configuration invalid: {reason}")]
    InvalidModuleConfig {
        /// Name of the module being validated.
        name: String,
        /// Human-readable reason.
        reason: String,
    },

    /// A module reference could not be resolved.
    #[error("Module {name} not found")]
    ModuleNotFound {
        /// Name of the missing module.
        name: String,
    },

    /// A field violated a structural constraint.
    #[error("Invalid module field {field}: {reason}")]
    InvalidField {
        /// Name of the offending field.
        field: String,
        /// Human-readable reason.
        reason: String,
    },

    /// Two version strings could not be compared.
    #[error("Version comparison failed: {reason}")]
    VersionComparisonFailed {
        /// Human-readable reason.
        reason: String,
    },

    /// Authentication credentials were rejected.
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed {
        /// Human-readable reason.
        reason: String,
    },

    /// The provided token was rejected as invalid or expired.
    #[error("Invalid or expired token")]
    InvalidToken,

    /// The provided token has expired.
    #[error("Token expired")]
    TokenExpired,

    /// The token signature does not match.
    #[error("Invalid token signature")]
    InvalidSignature,

    /// A required claim was missing from a JWT.
    #[error("Missing required claim: {claim}")]
    MissingClaim {
        /// Name of the missing claim.
        claim: String,
    },

    /// The `Authorization` header was malformed.
    #[error("Invalid authorization header")]
    InvalidAuthHeader,

    /// The token did not match the expected format.
    #[error("Invalid token format")]
    InvalidTokenFormat,

    /// The principal is unauthenticated.
    #[error("Unauthorized")]
    Unauthorized,

    /// The principal is authenticated but lacks permission.
    #[error("Forbidden: {reason}")]
    Forbidden {
        /// Human-readable reason.
        reason: String,
    },

    /// A user record was not found.
    #[error("User not found: {user_id}")]
    UserNotFound {
        /// Identifier of the missing user.
        user_id: String,
    },

    /// A session record was not found.
    #[error("Session not found: {session_id}")]
    SessionNotFound {
        /// Identifier of the missing session.
        session_id: String,
    },

    /// A session was rejected as invalid.
    #[error("Invalid session: {reason}")]
    InvalidSession {
        /// Human-readable reason.
        reason: String,
    },

    /// The session expiry has passed.
    #[error("Session expired")]
    SessionExpired,

    /// The session cookie was missing.
    #[error("Cookie not found")]
    CookieNotFound,

    /// The session cookie payload was malformed.
    #[error("Invalid cookie format")]
    InvalidCookieFormat,

    /// An underlying database call failed.
    #[error("Database error: {reason}")]
    DatabaseError {
        /// Human-readable reason.
        reason: String,
    },

    /// A required SQL table was missing.
    #[error("Table not found: {table}")]
    TableNotFound {
        /// Name of the missing table.
        table: String,
    },

    /// A schema introspection or migration step failed.
    #[error("Schema error: {message}")]
    SchemaError {
        /// Human-readable reason.
        message: String,
    },

    /// A required file was missing.
    #[error("File not found: {path}")]
    FileNotFound {
        /// Path that could not be located.
        path: String,
    },

    /// An underlying IO operation failed.
    #[error("IO error: {reason}")]
    IoError {
        /// Human-readable reason.
        reason: String,
    },

    /// Catch-all for unclassified internal failures.
    #[error("Internal server error: {reason}")]
    InternalError {
        /// Human-readable reason.
        reason: String,
    },
}

impl CoreError {
    /// Render this error as its `Display` representation.
    #[must_use]
    pub fn reason(&self) -> String {
        self.to_string()
    }

    /// Map this variant to the canonical HTTP status code that should
    /// surface to clients.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::AuthenticationFailed { .. }
            | Self::InvalidToken
            | Self::TokenExpired
            | Self::InvalidSignature
            | Self::Unauthorized
            | Self::SessionExpired => 401,
            Self::MissingClaim { .. }
            | Self::InvalidAuthHeader
            | Self::InvalidTokenFormat
            | Self::InvalidSession { .. }
            | Self::CookieNotFound
            | Self::InvalidCookieFormat
            | Self::MissingConfigField { .. }
            | Self::InvalidVersion { .. }
            | Self::InvalidModuleConfig { .. }
            | Self::InvalidField { .. }
            | Self::VersionComparisonFailed { .. } => 400,
            Self::Forbidden { .. } => 403,
            Self::UserNotFound { .. }
            | Self::SessionNotFound { .. }
            | Self::ModuleNotFound { .. }
            | Self::TableNotFound { .. }
            | Self::FileNotFound { .. } => 404,
            Self::DatabaseError { .. }
            | Self::SchemaError { .. }
            | Self::IoError { .. }
            | Self::InternalError { .. } => 500,
        }
    }

    /// True when the variant maps to a 4xx HTTP status.
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status_code() < 500
    }

    /// True when the variant maps to a 5xx HTTP status.
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status_code() >= 500
    }

    /// True for variants that indicate an authentication failure.
    #[must_use]
    pub const fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::AuthenticationFailed { .. }
                | Self::InvalidToken
                | Self::TokenExpired
                | Self::InvalidSignature
                | Self::InvalidAuthHeader
                | Self::InvalidTokenFormat
                | Self::Unauthorized
                | Self::SessionExpired
        )
    }

    /// True for variants that indicate a permission failure.
    #[must_use]
    pub const fn is_permission_error(&self) -> bool {
        matches!(self, Self::Forbidden { .. })
    }

    /// True for variants that indicate a missing resource.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::UserNotFound { .. }
                | Self::SessionNotFound { .. }
                | Self::ModuleNotFound { .. }
                | Self::TableNotFound { .. }
                | Self::FileNotFound { .. }
        )
    }
}

impl From<String> for CoreError {
    fn from(reason: String) -> Self {
        Self::InternalError { reason }
    }
}

impl From<&str> for CoreError {
    fn from(reason: &str) -> Self {
        Self::InternalError {
            reason: reason.to_string(),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            reason: err.to_string(),
        }
    }
}
