use anyhow::Result;

pub mod prompts;
pub mod repository;
pub mod resources;
pub mod server;
pub mod services;
pub mod tools;

pub use server::AdminServer;

pub async fn create_database_connection() -> Result<systemprompt::database::DbPool> {
    use systemprompt::system::AppContext;

    let ctx = AppContext::new().await?;
    Ok(ctx.db_pool().clone())
}
