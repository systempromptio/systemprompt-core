use thiserror::Error;

#[derive(Debug, Error)]
pub enum OauthError {
    #[error("provider error: {0}")]
    Provider(String),

    #[error("token error: {0}")]
    Token(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("repository error: {0}")]
    Repository(#[from] sqlx::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("config error: {0}")]
    Config(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type OauthResult<T> = Result<T, OauthError>;
