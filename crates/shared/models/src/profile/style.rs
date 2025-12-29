//! Profile style classification.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileStyle {
    Development,
    Production,
    Staging,
    Test,
    Custom,
}

impl ProfileStyle {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Development => "Dev",
            Self::Production => "Prod",
            Self::Staging => "Stage",
            Self::Test => "Test",
            Self::Custom => "Custom",
        }
    }
}
