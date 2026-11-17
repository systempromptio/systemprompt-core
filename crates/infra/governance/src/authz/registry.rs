//! The inputs a rule-based authz hook is wired from at the composition root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::CacheEpochs;

use super::parent_chain::ChainSources;
use super::subject::SubjectProviderSet;

/// The chain sources, cache epochs and subject providers a rule-based hook is
/// wired from at the composition root.
#[derive(Clone)]
pub struct AuthzWiring {
    pub sources: ChainSources,
    pub epochs: CacheEpochs,
    pub providers: SubjectProviderSet,
}

impl std::fmt::Debug for AuthzWiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthzWiring").finish_non_exhaustive()
    }
}
