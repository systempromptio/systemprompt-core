//! Environment detection and configuration.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
    Test,
}

impl Environment {
    pub fn detect() -> Self {
        if let Ok(env) = std::env::var("SYSTEMPROMPT_ENV") {
            return Self::from_string(&env);
        }

        if let Ok(env) = std::env::var("RAILWAY_ENVIRONMENT") {
            if env == "production" {
                return Self::Production;
            }
        }

        if let Ok(env) = std::env::var("NODE_ENV") {
            return Self::from_string(&env);
        }

        if std::env::var("DOCKER_CONTAINER").is_ok() {
            return Self::Production;
        }

        if cfg!(debug_assertions) {
            return Self::Development;
        }

        Self::Production
    }

    fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Self::Development,
            "test" | "testing" => Self::Test,
            _ => Self::Production,
        }
    }

    pub const fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }

    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    pub const fn is_test(&self) -> bool {
        matches!(self, Self::Test)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::detect()
    }
}
