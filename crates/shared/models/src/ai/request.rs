use super::response_format::StructuredOutputOptions;
use super::sampling::{ProviderConfig, SamplingParams};
use super::tools::McpTool;
use crate::execution::context::RequestContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiContentPart {
    Text { text: String },
    Image { mime_type: String, data: String },
    Audio { mime_type: String, data: String },
}

impl AiContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            mime_type: mime_type.into(),
            data: data.into(),
        }
    }

    pub fn audio(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Audio {
            mime_type: mime_type.into(),
            data: data.into(),
        }
    }

    pub const fn is_media(&self) -> bool {
        matches!(self, Self::Image { .. } | Self::Audio { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<AiContentPart>,
}

impl AiMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            parts: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            parts: Vec::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            parts: Vec::new(),
        }
    }

    pub fn user_with_parts(content: impl Into<String>, parts: Vec<AiContentPart>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            parts,
        }
    }

    pub fn has_media(&self) -> bool {
        self.parts.iter().any(AiContentPart::is_media)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    pub messages: Vec<AiMessage>,
    pub provider_config: ProviderConfig,
    pub context: RequestContext,
    pub sampling: Option<SamplingParams>,
    pub tools: Option<Vec<McpTool>>,
    pub structured_output: Option<StructuredOutputOptions>,
    pub system_prompt: Option<String>,
}

impl AiRequest {
    pub fn builder(
        messages: Vec<AiMessage>,
        provider: impl Into<String>,
        model: impl Into<String>,
        max_output_tokens: u32,
        context: RequestContext,
    ) -> AiRequestBuilder {
        AiRequestBuilder::new(messages, provider, model, max_output_tokens, context)
    }

    pub fn has_tools(&self) -> bool {
        self.tools.as_ref().is_some_and(|t| !t.is_empty())
    }

    pub fn provider(&self) -> &str {
        &self.provider_config.provider
    }

    pub fn model(&self) -> &str {
        &self.provider_config.model
    }

    pub const fn max_output_tokens(&self) -> u32 {
        self.provider_config.max_output_tokens
    }
}

#[derive(Debug)]
pub struct AiRequestBuilder {
    messages: Vec<AiMessage>,
    provider_config: ProviderConfig,
    context: RequestContext,
    sampling: Option<SamplingParams>,
    tools: Option<Vec<McpTool>>,
    structured_output: Option<StructuredOutputOptions>,
    system_prompt: Option<String>,
}

impl AiRequestBuilder {
    pub fn new(
        messages: Vec<AiMessage>,
        provider: impl Into<String>,
        model: impl Into<String>,
        max_output_tokens: u32,
        context: RequestContext,
    ) -> Self {
        Self {
            messages,
            provider_config: ProviderConfig::new(provider, model, max_output_tokens),
            context,
            sampling: None,
            tools: None,
            structured_output: None,
            system_prompt: None,
        }
    }

    pub fn with_sampling(mut self, sampling: SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub fn with_tools(mut self, tools: Vec<McpTool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_structured_output(mut self, options: StructuredOutputOptions) -> Self {
        self.structured_output = Some(options);
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn build(self) -> AiRequest {
        AiRequest {
            messages: self.messages,
            provider_config: self.provider_config,
            context: self.context,
            sampling: self.sampling,
            tools: self.tools,
            structured_output: self.structured_output,
            system_prompt: self.system_prompt,
        }
    }
}
