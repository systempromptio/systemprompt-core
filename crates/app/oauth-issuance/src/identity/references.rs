//! [`IdentitySecretReferences`]: the custom-secret names the identity member
//! reads — the generic OIDC provider's `client_secret_key` and every declared
//! OAuth client's `secret_key` — read from the services tree on every check,
//! so an edited declaration is covered at once. A file that cannot be read or
//! parsed references every name: an install's secret read is refused rather
//! than served past a declaration the check could not see.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_traits::SecretReferences;

use super::declaration::IdentityDocument;
use super::files::declaration_files;
use crate::sso::oidc::OidcConfig;

#[derive(Debug, Clone)]
pub struct IdentitySecretReferences {
    services_root: PathBuf,
}

impl IdentitySecretReferences {
    #[must_use]
    pub const fn over(services_root: PathBuf) -> Self {
        Self { services_root }
    }
}

fn referenced(services_root: &Path) -> Option<Vec<String>> {
    let mut names = Vec::new();
    if let Some(oidc) = OidcConfig::load(services_root).ok()? {
        names.push(oidc.client_secret_key);
    }
    for (_, path) in declaration_files(services_root).ok()?.unwrap_or_default() {
        let text = std::fs::read_to_string(path).ok()?;
        let doc: IdentityDocument = serde_yaml::from_str(&text).ok()?;
        names.extend(doc.oauth_clients.into_iter().filter_map(|c| c.secret_key));
    }
    Some(names)
}

impl SecretReferences for IdentitySecretReferences {
    fn references(&self, name: &str) -> bool {
        referenced(&self.services_root).is_none_or(|names| {
            names
                .iter()
                .any(|referenced| referenced.trim().eq_ignore_ascii_case(name))
        })
    }
}
