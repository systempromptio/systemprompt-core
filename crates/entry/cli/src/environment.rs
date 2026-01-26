#[derive(Debug, Clone, Copy)]
pub struct ExecutionEnvironment {
    pub is_fly: bool,
    pub is_remote_cli: bool,
}

impl ExecutionEnvironment {
    pub fn detect() -> Self {
        Self {
            is_fly: std::env::var("FLY_APP_NAME").is_ok(),
            is_remote_cli: std::env::var("SYSTEMPROMPT_CLI_REMOTE").is_ok(),
        }
    }
}
