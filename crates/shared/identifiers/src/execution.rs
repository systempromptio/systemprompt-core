//! Execution-trace identifiers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ExecutionStepId, generate);
crate::define_id!(LogId, generate, schema);
crate::define_id!(TokenId, generate);
crate::define_id!(ArtifactId, generate, schema);
