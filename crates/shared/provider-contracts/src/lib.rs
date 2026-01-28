mod component;
mod extender;
mod job;
mod llm;
mod page;
mod page_prerenderer;
mod template;
mod tool;
pub mod web_config;

pub use component::{ComponentContext, ComponentRenderer, RenderedComponent};
pub use extender::{ExtendedData, ExtenderContext, ExtenderContextBuilder, TemplateDataExtender};
pub use job::{Job, JobContext, JobResult};
pub use llm::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ChatStream, LlmProvider, LlmProviderError,
    LlmProviderResult, SamplingParameters, TokenUsage, ToolExecutionContext, ToolExecutor,
};
pub use page::{PageContext, PageDataProvider};
pub use page_prerenderer::{
    DynPagePrerenderer, PagePrepareContext, PagePrerenderer, PageRenderSpec,
};
pub use template::{TemplateDefinition, TemplateProvider, TemplateSource};
pub use tool::{
    ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolProvider,
    ToolProviderError, ToolProviderResult,
};
pub use web_config::{
    AnimationConfig, BrandingConfig, CardConfig, ColorsConfig, FontsConfig, FooterConfig,
    LayoutConfig, LogoConfig, MobileConfig, NavConfig, NavLink, NavigationConfig, PathsConfig,
    RadiusConfig, ScriptConfig, ShadowsConfig, SocialActionBar, SocialLink, SpacingConfig,
    TouchTargetsConfig, TypographyConfig, WebConfig, WebConfigError, ZIndexConfig,
};
