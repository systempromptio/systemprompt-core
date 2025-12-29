pub mod auth;
pub mod config;
pub mod error;
pub mod resilience;
pub mod slug;

pub use error::{AgentServiceError, Result};
pub type ServiceResult<T> = Result<T>;
pub use auth::{extract_bearer_token, AgentSessionUser, JwtClaims, JwtValidator};
pub use config::{
    AgentServiceConfig, ConfigValidation, ConnectionConfiguration, RuntimeConfiguration,
    RuntimeConfigurationBuilder, ServiceConfiguration,
};
pub use slug::{generate_slug, generate_unique_slug};
