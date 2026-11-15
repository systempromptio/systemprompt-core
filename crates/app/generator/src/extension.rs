//! [`GeneratorExtension`]: the generator's registration with the extension
//! loader.
//!
//! The extension has no schema, migration or provided state; prerendering
//! builds its templates from the extension providers each run. It owns the
//! `content_prerender` and `page_prerender` jobs, so a profile that disables
//! `generator` leaves both registered but inactive, and `infra jobs list`
//! names the owner. It depends on `content`, whose repositories and
//! prerender providers the jobs read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const GENERATOR_EXTENSION: &str = "generator";

/// The generator's extension: owner of the prerender jobs.
#[derive(Debug, Clone, Copy, Default)]
pub struct GeneratorExtension;

impl Extension for GeneratorExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: GENERATOR_EXTENSION,
            name: "Generator",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["content"]
    }
}

register_extension!(GeneratorExtension);
