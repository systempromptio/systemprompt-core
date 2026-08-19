//! `api` module — see crate-level docs for context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cli_gateway;
pub mod cloud;
pub mod contexts;
pub mod errors;
pub mod ext;
pub mod modules;
pub mod pagination;
pub mod responses;

pub use cli_gateway::{CliExecuteRequest, CliOutputEvent};
pub use cloud::{
    CloudApiError, CloudApiErrorDetail, CloudApiResponse, CloudCustomerInfo, CloudListResponse,
    CloudPlan, CloudPlanInfo, CloudStatusResponse, CloudTenant, CloudTenantInfo,
    CloudTenantSecrets, CloudTenantStatus, CloudTenantStatusResponse, CloudUserInfo,
    DeployResponse, RegistryToken, SetSecretsRequest, SubscriptionStatus, UserMeResponse,
};
pub use contexts::{
    ContextKind, CreateContextRequest, ParseContextKindError, UpdateContextRequest, UserContext,
    UserContextWithStats,
};
pub use errors::{ApiError, ErrorCode, ErrorResponse, ValidationError};
pub use ext::ApiErrorExt;
pub use modules::ModuleInfo;
pub use pagination::{
    ApiQuery, PaginationInfo, PaginationParams, SearchQuery, SortOrder, SortParams,
};
pub use responses::{
    AcceptedResponse, ApiResponse, CollectionResponse, CreatedResponse, DiscoveryResponse, Link,
    MarkdownFrontmatter, MarkdownResponse, ResponseLinks, ResponseMeta, SingleResponse,
    SuccessResponse,
};
