//! Long-running service lifecycle traits.

use async_trait::async_trait;

/// Lifecycle contract every long-lived service must satisfy.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Box<dyn Service>` by the runtime supervisor.
#[async_trait]
pub trait Service: Send + Sync {
    /// Stable identifier used in logs and supervision.
    fn name(&self) -> &str;

    /// Start the service. Implementations should be idempotent.
    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Request a graceful stop.
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Report whether the service is currently healthy.
    async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

/// Service variant that exposes a long-running `run` loop driven by the
/// supervisor.
#[async_trait]
pub trait AsyncService: Service {
    /// Execute the main service loop until completion or shutdown.
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
