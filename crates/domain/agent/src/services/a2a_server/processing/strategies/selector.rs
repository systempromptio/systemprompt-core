//! Execution-strategy selection per request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExecutionStrategy, PlannedAgenticStrategy, StandardExecutionStrategy};

#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionStrategySelector;

impl ExecutionStrategySelector {
    pub const fn new() -> Self {
        Self
    }

    pub fn select_strategy(has_tools: bool) -> Box<dyn ExecutionStrategy> {
        if has_tools {
            Box::new(PlannedAgenticStrategy::new())
        } else {
            Box::new(StandardExecutionStrategy::new())
        }
    }
}
