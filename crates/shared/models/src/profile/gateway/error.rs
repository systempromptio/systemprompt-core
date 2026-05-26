use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("Failed to read gateway catalog {path}: {source}")]
    CatalogRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse gateway catalog {path}: {source}")]
    CatalogParse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Invalid gateway catalog {path}: {source}")]
    CatalogInvalid {
        path: PathBuf,
        #[source]
        source: Box<Self>,
    },

    #[error("gateway catalog model has empty id")]
    ModelEmptyId,

    #[error("gateway catalog model '{model}' references unknown provider '{provider}'")]
    UnknownProvider { model: String, provider: String },

    #[error("gateway catalog provider has empty name")]
    ProviderEmptyName,

    #[error("gateway catalog provider '{name}' has empty endpoint")]
    ProviderEmptyEndpoint { name: String },

    #[error("gateway {label} endpoint '{endpoint}' is not permitted: {reason}")]
    BlockedEndpoint {
        label: String,
        endpoint: String,
        reason: String,
    },

    #[error(
        "gateway route '{route}' provider '{provider}' is not declared in the catalog providers"
    )]
    RouteProviderNotInCatalog { route: String, provider: String },

    #[error(
        "gateway route '{route}' endpoint '{route_endpoint}' disagrees with catalog provider \
         '{provider}' endpoint '{catalog_endpoint}'"
    )]
    RouteEndpointMismatch {
        route: String,
        provider: String,
        route_endpoint: String,
        catalog_endpoint: String,
    },

    #[error("gateway catalog model id or alias '{id}' is declared more than once")]
    DuplicateModelId { id: String },

    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: String },

    #[error("gateway catalog model '{model}' has no route whose pattern matches its id")]
    UnreachableModel { model: String },
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;
