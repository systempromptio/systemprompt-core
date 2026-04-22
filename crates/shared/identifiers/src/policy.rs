crate::define_id!(PolicyVersion);

impl PolicyVersion {
    pub fn unversioned() -> Self {
        Self("unversioned".to_string())
    }
}
