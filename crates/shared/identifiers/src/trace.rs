//! Distributed-tracing identifier with a `system` constant.

crate::define_id!(TraceId, generate, schema);

impl TraceId {
    /// Returns the canonical `"system"` trace identifier.
    pub fn system() -> Self {
        Self("system".to_string())
    }
}
