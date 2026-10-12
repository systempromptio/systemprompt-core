//! Headless-run identifiers.
//!
//! `HeadlessRunId` is the `headless_runs.id` the executor mints as a UUID;
//! `HostSessionId` is the session id a coding-agent host (Claude Code, Codex
//! CLI) reports for its own conversation, used to resume it. It is the host's
//! identity, not a systemprompt `SessionId`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(HeadlessRunId, uuid);
crate::define_id!(HostSessionId, non_empty);
