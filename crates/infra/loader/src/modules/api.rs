use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "api".into(),
        version: "0.0.1".into(),
        display_name: "API Gateway".into(),
        description: Some(
            "HTTP API server, request routing, authentication middleware, and service proxy".into(),
        ),
        weight: Some(-100),
        dependencies: vec!["users".into(), "oauth".into(), "log".into()],
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1".into()),
            openapi_path: Some("/api/docs".into()),
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "api-module-0001-0001-000000000001".into()
}
