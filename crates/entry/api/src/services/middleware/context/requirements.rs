use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextRequirement {
    None,
    UserOnly,
    #[default]
    UserWithContext,
    McpWithHeaders,
}

impl fmt::Display for ContextRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::UserOnly => write!(f, "user-only"),
            Self::UserWithContext => write!(f, "user-with-context"),
            Self::McpWithHeaders => write!(f, "mcp-with-headers"),
        }
    }
}
