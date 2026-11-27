//! Why a set of extensions could not be resolved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::engine::ExtensionId;

/// A set of extensions that cannot run together.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("two extensions named {id} share priority {priority}; neither shadows the other")]
    Ambiguous { id: ExtensionId, priority: u32 },
    #[error("extension {extension} depends on {dependency}, which is not linked")]
    MissingDependency {
        extension: ExtensionId,
        dependency: ExtensionId,
    },
    #[error("required extension {extension} depends on {dependency}, which is not required")]
    RequiredOnOptional {
        extension: ExtensionId,
        dependency: ExtensionId,
    },
    #[error("extension dependencies form a cycle: {}", chain_text(.chain))]
    DependencyCycle { chain: Vec<ExtensionId> },
}

fn chain_text(chain: &[ExtensionId]) -> String {
    chain
        .iter()
        .map(ExtensionId::as_str)
        .collect::<Vec<_>>()
        .join(" -> ")
}
