crate::define_id!(TraceId, generate, schema);

impl TraceId {
    pub fn system() -> Self {
        Self("system".to_string())
    }
}
