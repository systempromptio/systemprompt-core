//! Conversions from the kit-side marketplace references to the tolerant
//! mirrors the signed bridge manifest carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::manifest::{
    ManifestExternalMarketplace, ManifestExternalMarketplaceSource, ManifestExternalPlugin,
    ManifestExternalPluginSource,
};

use super::marketplace_external::{ExternalMarketplace, ExternalMarketplaceSource};
use super::marketplace_external_plugin::{ExternalPluginEntry, ExternalPluginSource};

impl From<ExternalMarketplace> for ManifestExternalMarketplace {
    fn from(external: ExternalMarketplace) -> Self {
        let source = match external.source {
            ExternalMarketplaceSource::Github { repo, reference } => {
                ManifestExternalMarketplaceSource {
                    source: "github".to_owned(),
                    repo: Some(repo),
                    url: None,
                    reference,
                }
            },
            ExternalMarketplaceSource::Git { url, reference } => {
                ManifestExternalMarketplaceSource {
                    source: "git".to_owned(),
                    repo: None,
                    url: Some(url),
                    reference,
                }
            },
        };
        Self {
            name: external.name,
            source,
        }
    }
}

impl From<ExternalPluginSource> for ManifestExternalPluginSource {
    fn from(source: ExternalPluginSource) -> Self {
        match source {
            ExternalPluginSource::Github {
                repo,
                reference,
                sha,
            } => Self {
                source: "github".to_owned(),
                repo: Some(repo),
                reference,
                sha: Some(sha),
                ..Self::default()
            },
            ExternalPluginSource::Url {
                url,
                reference,
                sha,
            } => Self {
                source: "url".to_owned(),
                url: Some(url),
                reference,
                sha: Some(sha),
                ..Self::default()
            },
            ExternalPluginSource::GitSubdir {
                url,
                path,
                reference,
                sha,
            } => Self {
                source: "git-subdir".to_owned(),
                url: Some(url),
                path: Some(path),
                reference,
                sha: Some(sha),
                ..Self::default()
            },
        }
    }
}

// Why: lint-ok: field-copy-from — `source` changes representation (the
// validated, tagged `ExternalPluginSource` becomes the flat tolerant wire
// object), and the kit type is `deny_unknown_fields` while this mirror must
// accept unknown keys.
impl From<ExternalPluginEntry> for ManifestExternalPlugin {
    fn from(entry: ExternalPluginEntry) -> Self {
        Self {
            name: entry.name,
            source: entry.source.into(),
            description: entry.description,
            version: entry.version,
            strict: entry.strict,
            skills: entry.skills,
        }
    }
}
