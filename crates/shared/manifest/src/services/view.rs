//! [`ServicesView`]: the owned services config and its resolved catalog, the
//! one handle every consumer and extension factory receives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_traits::ConfigWatch;

use super::ServicesConfig;
use super::catalog::ResolvedCatalog;

#[derive(Clone)]
pub struct ServicesView {
    pub config: ConfigWatch<ServicesConfig>,
    pub catalog: Arc<ResolvedCatalog>,
}

impl ServicesView {
    #[must_use]
    pub fn config(&self) -> Arc<ServicesConfig> {
        self.config.get()
    }
}

impl std::fmt::Debug for ServicesView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicesView").finish_non_exhaustive()
    }
}
