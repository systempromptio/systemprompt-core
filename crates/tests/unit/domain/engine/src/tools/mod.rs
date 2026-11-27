//! Tests for `systemprompt_engine::tools`, through its public API.

mod edit_tests;
mod error_tests;
mod fetch_tests;
mod glob_tests;
mod grep_tests;
#[cfg(unix)]
mod jobs_tests;
mod patch_tests;
mod path_tests;
mod read_tests;
mod repair_tests;
mod select_tests;
#[cfg(unix)]
mod shell_tests;
mod support;
mod todo_tests;
mod tokenize_tests;
mod truncate_tests;
