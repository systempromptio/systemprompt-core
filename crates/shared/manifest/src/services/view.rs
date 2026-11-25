//! The read-only services views.
//!
//! [`ServicesView`] is the owned services config and its resolved catalog,
//! the one handle every consumer and extension factory receives;
//! [`InstallServicesView`], the read-only services facts an install member
//! reads, a host-sealed seed listed in
//! `systemprompt_extension::reserved::INSTALL_SEEDS`.
//!
//! A [`ServicesView`] reads the live config and catalog and holds no handle
//! that writes or reloads them: the config watch it reads is private, and
//! only the services state that built it spawns the reload. An
//! [`InstallServicesView`] adds the active (synced) services root with its
//! provenance, the base services tree, the bundle cache directory and the
//! profile's bundle source list, each source as its name, transport and
//! location without user info, query or fragment; the name of the secret a
//! source authenticates with is not in it. It is everything an inventory
//! refresh reads (`systemprompt_marketplace::inventory::SourcesContext`), so
//! an install refreshes inventory without the profile. A fetch failure's text
//! never reaches the view: the provenance keeps its kind and composed hash,
//! and its error is a fixed sentence, since a fetch error can quote a source
//! URL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_traits::ConfigWatch;

use super::ServicesConfig;
use super::catalog::ResolvedCatalog;
use super::root::ActiveServicesRoot;
use crate::profile::{Profile, credential_free_url};

#[derive(Clone)]
pub struct ServicesView {
    config: ConfigWatch<ServicesConfig>,
    pub catalog: Arc<ResolvedCatalog>,
}

impl ServicesView {
    #[must_use]
    pub const fn over(config: ConfigWatch<ServicesConfig>, catalog: Arc<ResolvedCatalog>) -> Self {
        Self { config, catalog }
    }

    #[must_use]
    pub fn config(&self) -> Arc<ServicesConfig> {
        self.config.get()
    }

    #[must_use]
    pub fn reloads(&self) -> u64 {
        self.config.reloads()
    }
}

impl std::fmt::Debug for ServicesView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicesView").finish_non_exhaustive()
    }
}

/// How a bundle source is fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTransport {
    Https,
    Oci,
}

/// One bundle source of the profile, without its credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesSourceView {
    pub name: String,
    pub transport: SourceTransport,
    pub location: Option<String>,
}

fn install_root(root: &ActiveServicesRoot) -> ActiveServicesRoot {
    use super::root::ServicesProvenance;
    const WITHHELD: &str = "the bundle fetch failed; the server log has the cause";
    ActiveServicesRoot {
        path: root.path.clone(),
        base: root.base.clone(),
        provenance: match &root.provenance {
            ServicesProvenance::LastGood { composed_hash, .. } => ServicesProvenance::LastGood {
                composed_hash: composed_hash.clone(),
                error: WITHHELD.to_owned(),
            },
            ServicesProvenance::BundledFallback { .. } => ServicesProvenance::BundledFallback {
                error: WITHHELD.to_owned(),
            },
            other => other.clone(),
        },
    }
}

/// The read-only services facts an install member may read.
#[derive(Debug, Clone)]
pub struct InstallServicesView {
    services: ServicesView,
    root: ActiveServicesRoot,
    base_tree: PathBuf,
    cache_root: PathBuf,
    sources: Arc<[ServicesSourceView]>,
}

impl InstallServicesView {
    #[must_use]
    pub fn of(
        services: ServicesView,
        root: &ActiveServicesRoot,
        profile: &Profile,
        cache_root: PathBuf,
    ) -> Self {
        let sources = profile
            .services
            .sources
            .iter()
            .filter_map(|source| {
                let (transport, location) = match (&source.https, &source.oci) {
                    (Some(https), None) => {
                        (SourceTransport::Https, credential_free_url(&https.url))
                    },
                    (None, Some(oci)) => (SourceTransport::Oci, Some(oci.reference.clone())),
                    _ => return None,
                };
                Some(ServicesSourceView {
                    name: source.name.clone(),
                    transport,
                    location,
                })
            })
            .collect();
        Self {
            services,
            root: install_root(root),
            base_tree: PathBuf::from(&profile.paths.services),
            cache_root,
            sources,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root.path
    }

    #[must_use]
    pub const fn active_root(&self) -> &ActiveServicesRoot {
        &self.root
    }

    #[must_use]
    pub fn base_tree(&self) -> &Path {
        &self.base_tree
    }

    #[must_use]
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    #[must_use]
    pub fn config(&self) -> Arc<ServicesConfig> {
        self.services.config()
    }

    #[must_use]
    pub const fn catalog(&self) -> &Arc<ResolvedCatalog> {
        &self.services.catalog
    }

    #[must_use]
    pub fn sources(&self) -> &[ServicesSourceView] {
        &self.sources
    }
}
