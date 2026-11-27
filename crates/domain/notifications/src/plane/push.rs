//! The push declaration: which push providers this instance sends through.
//!
//! `services/notifications/push.yaml` names, per provider, the public
//! identifiers of the credential and the custom secret that holds its key
//! material. The file never holds a key. Apple needs the signing key's key id,
//! the team id, the app's bundle id as the topic and the environment; Google
//! needs the secret holding the service-account key. A provider the file does
//! not declare is not pushed to, and an instance with no file sends no push.
//!
//! The declaration is read once when the notification seams are built; it is
//! not projected into the database and is no entity of the
//! [`NotificationsPlane`](super::NotificationsPlane). The key material is
//! read from the secret store each time a provider token is minted, so a
//! rotated secret takes effect without a restart.
//!
//! [`PushSecretReferences`] reports the secret names the file references, so
//! no install member may declare or read them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SecretName;
use systemprompt_manifest::services::notifications::NOTIFICATIONS_DIR;
use systemprompt_models::devices::AppIdentifier;
use systemprompt_traits::SecretReferences;

pub const PUSH_FILE: &str = "push.yaml";

const APPLE_IDENTIFIER_LEN: usize = 10;
const APNS_PRODUCTION_ORIGIN: &str = "https://api.push.apple.com";
const APNS_SANDBOX_ORIGIN: &str = "https://api.sandbox.push.apple.com";

/// The providers `services/notifications/push.yaml` declares.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushDeclaration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns: Option<ApnsDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcm: Option<FcmDeclaration>,
}

/// Apple's push service: the token-signing key's identifiers, the app it
/// addresses and the secret holding the `.p8` key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApnsDeclaration {
    pub key_id: ApnsKeyId,
    pub team_id: ApnsTeamId,
    pub topic: AppIdentifier,
    pub environment: ApnsEnvironment,
    pub signing_key_secret: SecretName,
}

/// Google's push service: the secret holding the service-account key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FcmDeclaration {
    pub service_account_secret: SecretName,
}

/// Which of Apple's two push services the app's tokens were issued by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApnsEnvironment {
    Production,
    Sandbox,
}

impl ApnsEnvironment {
    #[must_use]
    pub const fn origin(self) -> &'static str {
        match self {
            Self::Production => APNS_PRODUCTION_ORIGIN,
            Self::Sandbox => APNS_SANDBOX_ORIGIN,
        }
    }
}

/// A string that is not one of Apple's ten-character identifiers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {field} {value:?}: must be 10 ASCII uppercase letters or digits")]
pub struct AppleIdentifierError {
    pub field: &'static str,
    pub value: String,
}

fn apple_identifier(field: &'static str, value: String) -> Result<String, AppleIdentifierError> {
    let valid = value.len() == APPLE_IDENTIFIER_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if valid {
        Ok(value)
    } else {
        Err(AppleIdentifierError { field, value })
    }
}

macro_rules! apple_identifier {
    ($(#[$doc:meta])* $name:ident, $field:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, AppleIdentifierError> {
                apple_identifier($field, value.into()).map(Self)
            }

            #[must_use]
            pub const fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl TryFrom<String> for $name {
            type Error = AppleIdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

apple_identifier!(ApnsKeyId, "APNs key id");
apple_identifier!(ApnsTeamId, "Apple team id");

/// Why `services/notifications/push.yaml` could not be used.
#[derive(Debug, thiserror::Error)]
pub enum PushDeclarationError {
    #[error("cannot read {}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse {}", .path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}

impl PushDeclaration {
    pub fn load(services_root: &Path) -> Result<Option<Self>, PushDeclarationError> {
        let path = services_root.join(NOTIFICATIONS_DIR).join(PUSH_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => return Err(PushDeclarationError::Read { path, source }),
        };
        serde_yaml::from_str(&text)
            .map(Some)
            .map_err(|source| PushDeclarationError::Parse { path, source })
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.apns.is_none() && self.fcm.is_none()
    }

    pub fn secret_names(&self) -> impl Iterator<Item = &SecretName> {
        self.apns
            .iter()
            .map(|apns| &apns.signing_key_secret)
            .chain(self.fcm.iter().map(|fcm| &fcm.service_account_secret))
    }
}

/// The custom-secret names the push declaration references.
///
/// They are read from the services tree on every check. A file that cannot
/// be read or parsed references every name, so an install's read is refused
/// instead of served past a declaration the check could not see.
#[derive(Debug, Clone)]
pub struct PushSecretReferences {
    services_root: Option<PathBuf>,
}

impl PushSecretReferences {
    #[must_use]
    pub const fn over(services_root: PathBuf) -> Self {
        Self {
            services_root: Some(services_root),
        }
    }

    #[must_use]
    pub const fn none() -> Self {
        Self {
            services_root: None,
        }
    }
}

impl SecretReferences for PushSecretReferences {
    fn references(&self, name: &str) -> bool {
        let Some(root) = &self.services_root else {
            return false;
        };
        match PushDeclaration::load(root) {
            Ok(Some(declaration)) => declaration
                .secret_names()
                .any(|referenced| referenced.as_str().trim().eq_ignore_ascii_case(name)),
            Ok(None) => false,
            Err(_) => true,
        }
    }
}
