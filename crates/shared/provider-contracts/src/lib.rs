//! Provider trait contracts for systemprompt.io.
//!
//! This crate defines the trait surface every provider implementation
//! (LLM, tool, RSS, sitemap, frontmatter, content-data, page-data,
//! page-prerender, component, template-data extender, job, template) is
//! expected to satisfy. Domain crates depend on these traits; concrete
//! providers live in higher-up crates and slot in via composition.
//!
//! # Public errors
//!
//! - LLM providers return [`llm::LlmProviderError`].
//! - Tool providers return [`tool::ToolProviderError`].
//! - All other providers return [`error::ProviderError`].
//!
//! # Feature flags
//!
//! This crate currently exposes no Cargo feature flags — all trait
//! contracts are always compiled. Provider implementations gate their
//! own SDK choices via features further up the stack.
//!
//! # Example
//!
//! ```no_run
//! use systemprompt_provider_contracts::llm::{ChatMessage, ChatRequest};
//!
//! let _request = ChatRequest::new(vec![ChatMessage::user("Hello")], "claude-sonnet-4-7", 1024);
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod component;
mod content_data;
mod error;
mod extender;
mod frontmatter;
mod job;
pub mod llm;
mod page;
mod page_prerenderer;
mod rss;
mod sitemap;
mod template;
mod tool;
pub mod web_config;

pub use component::{
    ComponentContext, ComponentRenderer, PartialSource, PartialTemplate, RenderedComponent,
};
pub use content_data::{ContentDataContext, ContentDataProvider};
pub use error::{ProviderError, ProviderResult};
pub use extender::{ExtendedData, ExtenderContext, ExtenderContextBuilder, TemplateDataExtender};
pub use frontmatter::{FrontmatterContext, FrontmatterProcessor};
pub use job::{Job, JobContext, JobResult};
pub use llm::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ChatStream, LlmProvider, LlmProviderError,
    LlmProviderResult, SamplingParameters, TokenUsage, ToolExecutionContext, ToolExecutor,
};
pub use page::{PageContext, PageDataProvider};
pub use page_prerenderer::{
    DynPagePrerenderer, PagePrepareContext, PagePrerenderer, PageRenderSpec,
};
pub use rss::{RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider, RssFeedSpec};
pub use sitemap::{
    PlaceholderMapping, SitemapAlternate, SitemapContext, SitemapProvider, SitemapSourceSpec,
    SitemapUrlEntry,
};
pub use template::{TemplateDefinition, TemplateProvider, TemplateSource};
pub use tool::{
    ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolProvider,
    ToolProviderError, ToolProviderResult,
};
pub use web_config::{
    AnimationConfig, BrandingConfig, CardConfig, ColorsConfig, FontsConfig, LayoutConfig,
    LogoConfig, MobileConfig, NavConfig, PathsConfig, RadiusConfig, ScriptConfig, ShadowsConfig,
    SiteI18nConfig, SocialActionBarConfig, SocialPlatform, SpacingConfig, TouchTargetsConfig,
    TypographyConfig, WebConfig, WebConfigError, ZIndexConfig,
};
