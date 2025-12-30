use std::path::PathBuf;
use systemprompt_models::modules::{Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "database".into(),
        version: "0.0.1".into(),
        display_name: "Database".into(),
        description: Some(
            "PostgreSQL database abstraction layer with type-safe query management".into(),
        ),
        weight: Some(-100),
        dependencies: vec![],
        schemas: Some(vec![ModuleSchema {
            table: String::new(),
            sql: SchemaSource::Inline(
                include_str!("../../../database/schema/functions.sql").into(),
            ),
            required_columns: vec![],
        }]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "database-module-0001-0001-000000000001".into()
}
