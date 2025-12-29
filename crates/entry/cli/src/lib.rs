//! SystemPrompt CLI Library
//!
//! This module exposes internal CLI components for testing purposes.
//! The main entry point is in `main.rs`.

#![allow(
    clippy::unused_async,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::missing_const_for_fn,
    clippy::clone_on_ref_ptr,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::option_if_let_else,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::single_match_else,
    clippy::if_not_else,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::separated_literal_suffix,
    clippy::match_same_arms,
    clippy::wildcard_imports,
    clippy::struct_field_names,
    clippy::similar_names,
    clippy::needless_raw_string_hashes,
    clippy::or_fun_call,
    clippy::print_stdout,
    clippy::struct_excessive_bools,
    clippy::redundant_closure,
    clippy::fn_params_excessive_bools,
    clippy::print_stderr,
    clippy::while_let_loop,
    clippy::needless_borrow,
    clippy::unnecessary_map_or,
    clippy::redundant_clone,
    clippy::ignored_unit_patterns,
    clippy::clone_on_copy,
    clippy::useless_vec,
    clippy::expect_used,
    clippy::use_self,
    clippy::module_inception,
    clippy::match_like_matches_macro,
    clippy::ref_option,
    clippy::assigning_clones,
    clippy::cast_lossless,
    clippy::incompatible_msrv
)]

pub mod cli_settings;
pub mod common;
