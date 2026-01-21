# systemprompt-ai

Multi-provider AI integration with tool execution, structured output, and request tracking.

## Overview

This module provides a unified interface for AI model interactions across multiple providers (Anthropic, OpenAI, Gemini) with support for:

- Text generation and streaming
- Tool/function calling with MCP integration
- Structured JSON output with schema validation
- Image generation
- Google Search grounding
- Request tracking and cost estimation

## API Layer

This is an internal service library with no HTTP endpoints. AI capabilities are exposed through consuming modules (primarily the Agent module).

## Dependencies

The AI module uses dependency injection for tool operations via the `ToolProvider` trait from `systemprompt-traits`. This allows:

- MCP module provides `McpToolProvider` for MCP-based tools
- AI module provides `NoopToolProvider` for non-tool use cases
- Other implementations possible without modifying AI module

## File Structure

```
src/
├── lib.rs                              # Public API exports
├── error.rs                            # AiError, RepositoryError types
├── jobs/
│   └── mod.rs                          # Background job definitions
├── models/
│   ├── mod.rs                          # AiRequest, AiRequestMessage, usage types
│   ├── ai_request_record.rs            # AiRequestRecord, RequestStatus, TokenInfo, builder
│   ├── image_generation.rs             # ImageGenerationRequest/Response, AspectRatio, Resolution
│   ├── message_converters.rs           # AiMessage to provider format conversions
│   └── providers/
│       ├── mod.rs                      # Provider model exports
│       ├── anthropic.rs                # Anthropic API DTOs (requests, responses, streaming)
│       ├── openai.rs                   # OpenAI API DTOs (requests, responses, tools)
│       └── gemini/
│           ├── mod.rs                  # Gemini model exports
│           ├── request.rs              # Gemini request types, function calling, tools
│           └── response.rs             # Gemini response types, grounding metadata
├── repository/
│   ├── mod.rs                          # Repository exports
│   └── ai_requests/
│       ├── mod.rs                      # AI request repository exports
│       ├── repository.rs               # AiRequestRepository struct, CreateAiRequest
│       ├── queries.rs                  # Read operations (get_by_id, usage queries)
│       ├── mutations.rs                # Write operations (create, update, insert)
│       └── message_operations.rs       # Message and tool call persistence
└── services/
    ├── mod.rs                          # Service exports
    ├── config/
    │   ├── mod.rs                      # Config exports
    │   └── validator.rs                # ConfigValidator for AI configuration
    ├── core/
    │   ├── mod.rs                      # Core service exports
    │   ├── image_service.rs            # ImageService for image generation
    │   ├── image_persistence.rs        # Image persistence and file record management
    │   ├── request_logging.rs          # Structured logging helpers
    │   ├── ai_service/
    │   │   ├── mod.rs                  # AiService exports
    │   │   ├── service.rs              # AiService struct, constructor, provider management
    │   │   ├── generation.rs           # generate() - basic text generation
    │   │   ├── tool_execution.rs       # generate_with_tools(), execute_tools()
    │   │   ├── streaming.rs            # generate_stream(), health_check()
    │   │   ├── planning.rs             # generate_plan(), generate_response(), cost estimation
    │   │   └── provider_impl.rs        # AiProvider trait implementation for AiService
    │   └── request_storage/
    │       ├── mod.rs                  # Storage exports
    │       ├── storage.rs              # RequestStorage, StoreParams
    │       ├── record_builder.rs       # build_record(), extract_messages/tool_calls
    │       └── async_operations.rs     # Background persistence operations
    ├── providers/
    │   ├── mod.rs                      # Provider exports
    │   ├── provider_trait.rs           # AiProvider trait, GenerationParams variants
    │   ├── provider_factory.rs         # ProviderFactory for creating providers
    │   ├── image_provider_trait.rs     # ImageProvider trait, capabilities
    │   ├── image_provider_factory.rs   # ImageProviderFactory
    │   ├── gemini_images.rs            # GeminiImageProvider implementation
    │   ├── gemini_images_helpers.rs    # Request building and response parsing helpers
    │   ├── openai_images.rs            # OpenAiImageProvider implementation
    │   ├── shared/
    │   │   ├── mod.rs                  # Shared utilities exports
    │   │   ├── http_client.rs          # HTTP client helpers
    │   │   └── response_builder.rs     # Response building utilities
    │   ├── anthropic/
    │   │   ├── mod.rs                  # Anthropic provider exports
    │   │   ├── provider.rs             # AnthropicProvider struct
    │   │   ├── converters.rs           # Message/tool conversion
    │   │   ├── generation.rs           # generate(), generate_with_tools()
    │   │   ├── streaming.rs            # Streaming implementation
    │   │   ├── thinking.rs             # Extended thinking support
    │   │   └── trait_impl.rs           # AiProvider trait implementation
    │   ├── openai/
    │   │   ├── mod.rs                  # OpenAI provider exports
    │   │   ├── provider.rs             # OpenAiProvider struct
    │   │   ├── converters.rs           # Message/tool/format conversion
    │   │   ├── generation.rs           # generate(), generate_with_tools(), structured
    │   │   ├── streaming.rs            # Streaming implementation
    │   │   ├── response_builder.rs     # Response building helpers
    │   │   ├── reasoning.rs            # Reasoning effort configuration
    │   │   └── trait_impl.rs           # AiProvider trait implementation
    │   └── gemini/
    │       ├── mod.rs                  # Gemini provider exports
    │       ├── constants.rs            # API constants, timeouts
    │       ├── provider.rs             # GeminiProvider struct
    │       ├── converters.rs           # Message/content conversion
    │       ├── request_builders.rs     # Request building helpers
    │       ├── params.rs               # ToolRequestParams, ToolResultParams builders
    │       ├── tool_conversion.rs      # Tool schema transformation
    │       ├── tools.rs                # generate_with_tools(), tool result handling
    │       ├── generation.rs           # generate(), generate_with_schema()
    │       ├── streaming.rs            # Streaming implementation
    │       ├── search.rs               # Google Search grounding
    │       ├── code_execution.rs       # Code execution support
    │       └── trait_impl.rs           # AiProvider trait implementation
    ├── schema/
    │   ├── mod.rs                      # Schema service exports
    │   ├── analyzer.rs                 # DiscriminatedUnion detection
    │   ├── capabilities.rs             # ProviderCapabilities
    │   ├── mapper.rs                   # ToolNameMapper for split tools
    │   ├── sanitizer.rs                # SchemaSanitizer for provider compatibility
    │   └── transformer.rs              # SchemaTransformer, TransformedTool
    ├── storage/
    │   ├── mod.rs                      # Storage exports
    │   └── image_storage.rs            # ImageStorage, StorageConfig
    ├── structured_output/
    │   ├── mod.rs                      # StructuredOutputProcessor
    │   ├── parser.rs                   # JSON extraction from responses
    │   └── validator.rs                # Schema validation
    ├── tooled/
    │   ├── mod.rs                      # Tooled service exports
    │   ├── executor.rs                 # TooledExecutor, ResponseStrategy
    │   ├── formatter.rs                # ToolResultFormatter
    │   └── synthesizer.rs              # ResponseSynthesizer, fallback handling
    └── tools/
        ├── mod.rs                      # Tool service exports
        ├── adapter.rs                  # Type conversions (McpTool <-> ToolDefinition)
        ├── discovery.rs                # ToolDiscovery (uses ToolProvider trait)
        └── noop_provider.rs            # NoopToolProvider for non-tool use cases
```

## Module Descriptions

### `models/`
Data structures for AI requests, responses, and provider-specific DTOs. Contains conversion logic for translating between the unified `AiMessage` format and provider-specific formats.

### `repository/`
Database access layer for persisting AI requests, messages, and tool calls. Uses SQLX macros for compile-time query verification.

### `services/core/`
Core AI functionality including `AiService` (main entry point) and `ImageService` for image generation. Handles request orchestration, storage, and logging.

### `services/providers/`
Provider-specific implementations for Anthropic, OpenAI, and Gemini. Each provider implements the `AiProvider` trait for consistent API access.

### `services/schema/`
Schema transformation for tool definitions. Handles discriminated union splitting for providers that don't support `anyOf` schemas (like Gemini).

### `services/tooled/`
Tool execution orchestration. Executes tool calls via the injected `ToolProvider` and synthesizes responses from tool results.

### `services/structured_output/`
JSON extraction and schema validation for structured output responses.

## Database

Tables: `ai_requests`, `ai_request_messages`, `ai_request_tool_calls`

## Usage

```rust
use systemprompt_ai::{AiService, AiRequest, AiMessage, NoopToolProvider};
use systemprompt_database::DbPool;
use std::sync::Arc;

let tool_provider = Arc::new(NoopToolProvider::new());
let ai_service = AiService::new(db_pool, &ai_config, tool_provider)?;

let request = AiRequest::builder(
    vec![AiMessage::user("Hello!")],
    "gemini",
    "gemini-2.5-flash",
    8192,
    context,
)
.build();

let response = ai_service.generate(&request).await?;
```
