//! Names of the tools the prompts mention.
//!
//! The tools crate and the engine register tools under these names;
//! a tool registered under another name is not referred to correctly by the
//! prompt text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const SHELL: &str = "shell";
pub const READ: &str = "read";
pub const WRITE: &str = "write";
pub const EDIT: &str = "edit";
pub const PATCH: &str = "patch";
pub const GLOB: &str = "glob";
pub const GREP: &str = "grep";
pub const FETCH: &str = "fetch";
pub const TODO: &str = "todo";
pub const TASK: &str = "task";
pub const SKILL: &str = "skill";
pub const QUESTION: &str = "question";

pub const ALL: &str = "*";

pub const FILE_WRITERS: &[&str] = &[WRITE, EDIT, PATCH];
