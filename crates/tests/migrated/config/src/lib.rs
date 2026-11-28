//! Behaviour tests for the engine's configuration: settings layering,
//! permission rules, agents, skills, slash commands, front matter and
//! instruction files, migrated case by case from the reference suite (batch 2
//! of the parity programme). Each test states one behaviour in the harness's
//! own terms; the link from a test to the case it carries lives in `cases.md`
//! beside this crate, never in the test itself. A test marked
//! `#[ignore = "parity ledger: …"]` asserts the reference behaviour where the
//! harness differs.

#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod frontmatter_tests;
#[cfg(test)]
mod instruction_tests;
#[cfg(test)]
mod permission_tests;
#[cfg(test)]
mod reload_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod skill_tests;
#[cfg(test)]
mod support;
