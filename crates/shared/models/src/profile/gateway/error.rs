use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: String },

    #[error("gateway route '{route}' provider '{provider}' is not declared in profile.providers")]
    RouteProviderNotInRegistry { route: String, provider: String },

    #[error("gateway default_provider '{provider}' is not declared in profile.providers")]
    DefaultProviderNotInRegistry { provider: String },
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;
