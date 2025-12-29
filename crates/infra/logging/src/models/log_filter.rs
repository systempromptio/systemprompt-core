#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    page: i32,
    per_page: i32,
    level: Option<String>,
    module: Option<String>,
    message: Option<String>,
}

impl LogFilter {
    #[must_use]
    pub const fn new(page: i32, per_page: i32) -> Self {
        Self {
            page,
            per_page,
            level: None,
            module: None,
            message: None,
        }
    }

    #[must_use]
    pub fn with_level(mut self, level: impl Into<String>) -> Self {
        self.level = Some(level.into());
        self
    }

    #[must_use]
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    #[must_use]
    pub const fn page(&self) -> i32 {
        self.page
    }

    #[must_use]
    pub const fn per_page(&self) -> i32 {
        self.per_page
    }

    #[must_use]
    pub fn level(&self) -> Option<&str> {
        self.level.as_deref()
    }

    #[must_use]
    pub fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}
