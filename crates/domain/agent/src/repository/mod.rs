use systemprompt_database::DbPool;

pub mod agent_service;
pub mod content;
pub mod context;
pub mod execution;
pub mod task;

pub use context::ContextRepository;
pub use systemprompt_traits::RepositoryError;

#[derive(Debug)]
pub struct A2ARepositories {
    db_pool: DbPool,
    pub agent_services: agent_service::AgentServiceRepository,
    pub tasks: task::TaskRepository,
    pub execution_steps: execution::ExecutionStepRepository,
    pub push_notification_configs: content::PushNotificationConfigRepository,
}

impl A2ARepositories {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let agent_services = agent_service::AgentServiceRepository::new(db)?;
        let tasks = task::TaskRepository::new(db)?;
        let execution_steps = execution::ExecutionStepRepository::new(db)?;
        let push_notification_configs = content::PushNotificationConfigRepository::new(db)?;

        Ok(Self {
            db_pool: db.clone(),
            agent_services,
            tasks,
            execution_steps,
            push_notification_configs,
        })
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }
}
