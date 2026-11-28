//! Behaviour tests for the engine's sessions and store: the agent loop,
//! steering and queueing, interruption, retries, compaction, titles,
//! subagents, revert, permission requests during a run, the store and its
//! change feed. Migrated case by case from the reference suite (batch 2 of the
//! parity programme). Tests that need a model run the engine in-process
//! against the scripted instance on a loopback port; nothing reaches a real
//! instance or model. Each test states one behaviour in the harness's own
//! terms; the link from a test to the case it carries lives in `cases.md`
//! beside this crate, never in the test itself. A test marked
//! `#[ignore = "parity ledger: …"]` asserts the reference behaviour where the
//! harness differs.

#[cfg(test)]
mod ask_tests;
#[cfg(test)]
mod compaction_run_tests;
#[cfg(test)]
mod compaction_tests;
#[cfg(test)]
mod feed_tests;
#[cfg(test)]
mod loop_tests;
#[cfg(test)]
mod request_mapping_tests;
#[cfg(test)]
mod retry_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod subagent_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod title_tests;
#[cfg(test)]
mod tools_tests;
