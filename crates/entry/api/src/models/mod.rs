//! HTTP server request/response models for the API entry point.
//!
//! Holds the bind-address configuration ([`ServerConfig`]) consumed when
//! standing up the axum listener.

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_owned(),
            port: 8080,
        }
    }
}
