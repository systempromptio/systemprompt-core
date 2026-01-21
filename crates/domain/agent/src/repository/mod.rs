use systemprompt_database::DbPool;

pub mod agent_service;
pub mod content;
pub mod context;
pub mod execution;
pub mod task;

pub use context::ContextRepository;
pub use systemprompt_traits::RepositoryError;

pub trait Repository {
    fn pool(&self) -> &DbPool;
}

#[derive(Debug)]
pub struct A2ARepositories {
    db_pool: DbPool,
    pub agent_services: agent_service::AgentServiceRepository,
    pub tasks: task::TaskRepository,
    pub execution_steps: execution::ExecutionStepRepository,
    pub push_notification_configs: content::PushNotificationConfigRepository,
}

impl A2ARepositories {
    pub async fn new(database_url: &str) -> Result<Self, RepositoryError> {
        use std::sync::Arc;
        use systemprompt_database::Database;

        let db_pool = Database::new_postgres(database_url)
            .await
            .map_err(RepositoryError::Other)?;
        let db_pool = Arc::new(db_pool);

        let agent_services = agent_service::AgentServiceRepository::new(db_pool.clone());
        let tasks = task::TaskRepository::new(db_pool.clone());
        let execution_steps = execution::ExecutionStepRepository::new(&db_pool)?;
        let push_notification_configs = content::PushNotificationConfigRepository::new(&db_pool)?;

        Ok(Self {
            db_pool,
            agent_services,
            tasks,
            execution_steps,
            push_notification_configs,
        })
    }

    #[must_use]
    pub const fn pool(&self) -> &DbPool {
        &self.db_pool
    }

    pub fn from_pool(db_pool: DbPool) -> Result<Self, RepositoryError> {
        let agent_services = agent_service::AgentServiceRepository::new(db_pool.clone());
        let tasks = task::TaskRepository::new(db_pool.clone());
        let execution_steps = execution::ExecutionStepRepository::new(&db_pool)?;
        let push_notification_configs = content::PushNotificationConfigRepository::new(&db_pool)?;

        Ok(Self {
            db_pool,
            agent_services,
            tasks,
            execution_steps,
            push_notification_configs,
        })
    }
}
