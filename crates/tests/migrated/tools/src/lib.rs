//! Behaviour tests for the engine's file, search, patch and shell tools,
//! migrated case by case from the reference suite (batch 1 of the parity
//! programme). Each test states one behaviour in the harness's own terms; the
//! link from a test to the case it carries lives in `cases.md` beside this
//! crate, never in the test itself. A test marked `#[ignore = "parity
//! ledger: …"]` asserts the reference behaviour where the harness differs.

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod fetch_convert_tests;
#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod output_tests;
#[cfg(test)]
mod patch_tests;
#[cfg(test)]
mod paths_tests;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod scan;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod shell;
#[cfg(test)]
mod support;
