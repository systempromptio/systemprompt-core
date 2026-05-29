//! Long-running service lifecycle traits.
//!
//! Dispatched as trait objects (`dyn _`), so they use `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.

use async_trait::async_trait;

#[async_trait]
pub trait Service: Send + Sync {
    fn name(&self) -> &str;

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
pub trait AsyncService: Service {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
