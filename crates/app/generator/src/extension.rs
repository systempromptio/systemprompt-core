//! [`GeneratorExtension`]: the generator member of the composition.
//!
//! The member has no schema, migration or provided state; prerendering
//! builds its template registry each run from the content hooks the
//! `content::contract::ContentSlot` builds, which the member requires, so a
//! profile that disables `content` refuses `generator`. It contributes the
//! `content_prerender` and `page_prerender` jobs, so a profile that disables
//! `generator` admits neither, and `infra jobs list` names the owner. The
//! content prerender orders its popular lists by the content popularity the
//! analytics member provides, when the host holds one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_content::ContentHooks;
use systemprompt_extension::prelude::*;

use crate::jobs::{ContentPrerenderJob, PagePrerenderJob};

pub const GENERATOR_EXTENSION: &str = "generator";

/// The generator's member: owner of the prerender jobs.
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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.requires::<Arc<ContentHooks>>()
            .job(ContentPrerenderJob)
            .job(PagePrerenderJob);
        Ok(())
    }
}
