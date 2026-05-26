//! Unit tests for execution strategies selector and Default impls.
//!
//! Targets:
//! - crates/domain/agent/src/services/a2a_server/processing/strategies/selector.rs
//! - crates/domain/agent/src/services/a2a_server/processing/strategies/mod.rs
//! - crates/domain/agent/src/services/a2a_server/processing/strategies/standard.rs

use systemprompt_agent::services::a2a_server::processing::strategies::{
    ExecutionResult, ExecutionStrategy, ExecutionStrategySelector, StandardExecutionStrategy,
};

#[test]
fn execution_result_default() {
    let result = ExecutionResult::default();
    assert_eq!(result.accumulated_text, "");
    assert!(result.tool_calls.is_empty());
    assert!(result.tool_results.is_empty());
    assert!(result.tools.is_empty());
    assert_eq!(result.iterations, 1);
}

#[test]
fn execution_strategy_selector_new_no_tools_selects_standard() {
    let _selector = ExecutionStrategySelector::new();
    let strategy = ExecutionStrategySelector::select_strategy(false);
    assert_eq!(strategy.name(), "standard");
}

#[test]
fn execution_strategy_selector_with_tools_selects_planned() {
    let strategy = ExecutionStrategySelector::select_strategy(true);
    assert_eq!(strategy.name(), "planned");
}

#[test]
fn standard_strategy_new_and_default() {
    let s = StandardExecutionStrategy::new();
    assert_eq!(s.name(), "standard");

    let d = StandardExecutionStrategy::default();
    assert_eq!(d.name(), "standard");
}

#[test]
fn execution_strategy_selector_default() {
    let _selector = ExecutionStrategySelector;
    let _selector2 = ExecutionStrategySelector::default();
}
