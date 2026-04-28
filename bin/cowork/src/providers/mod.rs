use crate::types::HelperOutput;

pub mod mtls;
pub mod pat;
pub mod session;

#[derive(Debug)]
pub enum AuthError {
    NotConfigured,
    Failed(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "not configured"),
            Self::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

pub trait AuthProvider {
    fn name(&self) -> &'static str;
    fn authenticate(&self) -> Result<HelperOutput, AuthError>;
}
