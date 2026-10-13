//! Errors raised while resolving, loading, validating or reloading secrets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::ProfileError;
use systemprompt_models::errors::SecretsError;

use super::vault::VaultError;
use crate::bootstrap::key_material::KeyMaterialError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecretsBootstrapError {
    #[error(
        "Secrets not initialized. Call SecretsBootstrap::init() after ProfileBootstrap::init()"
    )]
    NotInitialized,

    #[error("Secrets already initialized")]
    AlreadyInitialized,

    #[error("Profile not initialized. Call ProfileBootstrap::init() first")]
    ProfileNotInitialized,

    #[error("Secrets file not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid secrets file: {0}")]
    InvalidSecretsFile(#[source] SecretsError),

    #[error("Secrets document could not be encoded: {0}")]
    DocumentEncode(#[source] serde_json::Error),

    #[error("No secrets configured. Create a secrets.json file.")]
    NoSecretsConfigured,

    #[error("Invalid secrets configuration in profile: {0}")]
    SecretsConfigInvalid(#[source] ProfileError),

    #[error(
        "secrets.source is 'vault' but the profile has no secrets.vault block. Add one or switch \
         secrets.source to 'file' or 'env'."
    )]
    VaultBlockMissing,

    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error(
        "OAuth at-rest pepper is required. Add 'oauth_at_rest_pepper' (>= 32 chars) to your \
         secrets file or set OAUTH_AT_REST_PEPPER environment variable."
    )]
    OauthAtRestPepperRequired,

    #[error(
        "Database URL is required. Add 'database_url' to your secrets.json or set DATABASE_URL \
         environment variable."
    )]
    DatabaseUrlRequired,

    #[error(
        "manifest_signing_secret_seed is required: every replica must share one seed, so it is \
         never generated at boot. Run `systemprompt admin identity generate --json` once and \
         distribute the value (secrets file or MANIFEST_SIGNING_SECRET_SEED)."
    )]
    ManifestSeedRequired,

    #[error(
        "signing_key_pem is required on cloud and deployment-host boots: every replica must \
         sign with one key, so it is never read from a file beside the binary there. Run \
         `systemprompt admin identity generate --json` once and distribute the value (secrets \
         file or SIGNING_KEY_PEM)."
    )]
    SigningKeyPemRequired,

    #[error("manifest_signing_secret_seed is invalid: {0}")]
    ManifestSeedInvalid(#[source] KeyMaterialError),

    #[error(
        "encryption_master_key is required: it seals at-rest secrets and the gateway accounting \
         journal. Add 64 hex characters (`openssl rand -hex 32`) as 'encryption_master_key' in \
         secrets.json, or take the value from `systemprompt admin identity generate`. With \
         `secrets.source: env`, also list it in SYSTEMPROMPT_CUSTOM_SECRETS."
    )]
    EncryptionMasterKeyRequired,

    #[error(
        "encryption_master_key is invalid ({0}): it must be 32 bytes as 64 hex characters \
         (`openssl rand -hex 32`)"
    )]
    EncryptionMasterKeyInvalid(#[source] KeyMaterialError),

    #[error("signing_key_pem secret is invalid: {0}")]
    SigningKeyPemInvalid(#[source] KeyMaterialError),

    #[error(
        "secrets.source is 'external' but the profile has no secrets.external block. Add one or \
         switch secrets.source to 'file', 'env' or 'vault'."
    )]
    ExternalBlockMissing,

    #[error(
        "secrets.external.kind '{kind}' is not registered in this binary (registered: {known:?})"
    )]
    UnknownExternalKind { kind: String, known: Vec<String> },

    #[error("external secrets source '{kind}' failed: {source}")]
    External {
        kind: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error(
        "secrets reload rejected: '{key}' changed, and it is pinned for the life of the process. \
         Restart to adopt a new value."
    )]
    ReloadRejected { key: String },
}
