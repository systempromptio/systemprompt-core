//! Behaviour tests for the engine's MCP client, migrated case by case from the
//! reference suite (batch 3 of the parity programme): newline-delimited
//! framing, paged listings, tool calls and their results, resources, prompts,
//! timeouts and the manager's lifecycle. The link from a test to the case it
//! carries lives in `cases.md` beside this crate, never in the test itself.
//! A test marked `#[ignore = "parity ledger: …"]` asserts the reference
//! behaviour where the harness differs.

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod framing_tests;
#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod support;
