//! The consolidation seam: an extension-supplied [`FactConsolidator`] reads
//! the stored facts of one kind and subject and proposes the facts that
//! should replace or summarise them.
//!
//! Scaffolding, inactive unless an extension opts in: core registers no
//! consolidator, the daily `brain_consolidate` job runs only when
//! `brain.consolidation.enabled` is `true` and at least one consolidator is
//! registered, and its candidates land as `proposed` facts for a person to
//! confirm. The model client in the input is the only way a consolidator
//! reaches a model, so every call is metered and audited through the
//! gateway under the job's token budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_models::brain::{BrainFact, FactCandidate, KindDecl, SubjectKey};

use super::errors::BrainConsolidateError;
use super::traits::BrainModelClient;

/// What one consolidator is asked to read: the facts of one kind about one
/// subject, the model client it may call and the tokens it may spend.
pub struct ConsolidationInput {
    pub kind: KindDecl,
    pub subject: SubjectKey,
    pub facts: Vec<BrainFact>,
    pub model: Arc<dyn BrainModelClient>,
    /// Tokens left in the job's daily budget when this input was built; a
    /// consolidator stays under it.
    pub budget_tokens: u64,
}

impl fmt::Debug for ConsolidationInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsolidationInput")
            .field("kind", &self.kind.kind)
            .field("subject", &self.subject)
            .field("facts", &self.facts.len())
            .field("budget_tokens", &self.budget_tokens)
            .finish_non_exhaustive()
    }
}

/// Proposes consolidated facts for the kinds it `handles`; held as
/// `Arc<dyn FactConsolidator>`. The candidates it returns are entered as
/// `proposed` facts of the kind they name.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactConsolidator: Send + Sync {
    fn name(&self) -> &'static str;

    fn handles(&self, kind: &KindDecl) -> bool;

    async fn consolidate(
        &self,
        input: &ConsolidationInput,
    ) -> Result<Vec<FactCandidate>, BrainConsolidateError>;
}
