#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("hash mismatch for {what}: expected {expected}, got {actual}")]
    HashMismatch {
        what: String,
        expected: String,
        actual: String,
    },
    #[error("unsafe path in manifest: {0}")]
    UnsafePath(String),
    #[error("network: {0}")]
    Network(String),
    #[error("io error in {context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    #[error("serialize {what}: {source}")]
    Serialize {
        what: String,
        source: serde_json::Error,
    },
    #[error("{0}")]
    Detail(String),
}

impl From<String> for ApplyError {
    fn from(s: String) -> Self {
        ApplyError::Detail(s)
    }
}
