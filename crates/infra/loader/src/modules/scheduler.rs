use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "scheduler".into(),
        version: "0.0.1".into(),
        display_name: "Task Scheduler".into(),
        description: Some(
            "Infrastructure module for scheduling background jobs, cron tasks, and agentic \
             workflows"
                .into(),
        ),
        weight: Some(5),
        dependencies: vec!["log".into(), "ai".into(), "agent".into()],
        schemas: Some(vec![ModuleSchema {
            table: "scheduled_jobs".into(),
            sql: SchemaSource::Inline(
                include_str!("../../../../app/scheduler/schema/scheduled_jobs.sql").into(),
            ),
            required_columns: vec![
                "id".into(),
                "job_name".into(),
                "schedule".into(),
                "enabled".into(),
                "last_run".into(),
                "next_run".into(),
            ],
        }]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: false,
            path_prefix: Some("/api/v1/scheduler".into()),
            openapi_path: None,
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "scheduler-module-0001-0001-000000000001".into()
}
