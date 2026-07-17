//! Distributed-tracing identifier with a `system` constant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(TraceId, generate, schema);

impl TraceId {
    pub fn system() -> Self {
        Self("system".to_owned())
    }
}
