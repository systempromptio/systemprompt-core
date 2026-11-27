//! What an extension declares about itself before it contributes anything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::engine::ExtensionId;

/// An extension's identity and place among the others.
///
/// `priority` orders extensions (highest first) and decides which of two
/// extensions with the same id is kept: the higher one shadows the lower, which
/// is how a white-label build replaces a stock extension. A `required`
/// extension runs whatever the instance enables, is never shadowed by one that
/// is not required, and depends only on other required extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionMetadata {
    pub id: ExtensionId,
    pub name: &'static str,
    pub priority: u32,
    pub dependencies: Vec<ExtensionId>,
    pub required: bool,
}

impl ExtensionMetadata {
    pub const DEFAULT_PRIORITY: u32 = 100;

    pub const fn new(id: ExtensionId, name: &'static str) -> Self {
        Self {
            id,
            name,
            priority: Self::DEFAULT_PRIORITY,
            dependencies: Vec::new(),
            required: false,
        }
    }

    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn depends_on(mut self, dependency: ExtensionId) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub const fn mark_required(mut self) -> Self {
        self.required = true;
        self
    }
}
