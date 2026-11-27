//! Tests for harness-tools, through its public API.

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod glob_tests;
#[cfg(test)]
mod grep_tests;
#[cfg(all(test, unix))]
mod jobs_tests;
#[cfg(test)]
mod patch_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod repair_tests;
#[cfg(test)]
mod select_tests;
#[cfg(all(test, unix))]
mod shell_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod todo_tests;
#[cfg(test)]
mod tokenize_tests;
#[cfg(test)]
mod truncate_tests;
