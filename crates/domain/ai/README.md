# AI Module

Multi-provider AI integration with tool execution, structured output, and request tracking.

## API Layer

This module is an internal service library with no HTTP endpoints. AI capabilities are exposed through consuming modules (primarily the Agent module).

## Dependencies

The AI module uses dependency injection for tool operations via the `ToolProvider` trait from `systemprompt-traits`. This allows:

- MCP module provides `McpToolProvider` for MCP-based tools
- AI module provides `NoopToolProvider` for non-tool use cases
- Other implementations possible without modifying AI module

## Structure

```
src/
├── lib.rs                          # Public API exports
├── error.rs                        # AiError, RepositoryError types
├── jobs/
│   ├── mod.rs
│   ├── evaluate_conversations.rs   # EvaluateConversationsJob
│   ├── evaluation_helpers.rs       # Conversation evaluation logic
│   └── evaluation_prompt.rs        # Evaluation prompt template
├── models/
│   ├── mod.rs                      # AiRequest, AiRequestMessage, usage types
│   ├── ai_request_record.rs        # AiRequestRecord, RequestStatus, TokenInfo
│   ├── image_generation.rs         # ImageGenerationRequest/Response types
│   ├── message_converters.rs       # AiMessage to provider format conversions
│   └── providers/
│       ├── mod.rs
│       ├── anthropic.rs            # Anthropic API DTOs
│       ├── openai.rs               # OpenAI API DTOs
│       └── gemini/
│           ├── mod.rs
│           ├── request.rs          # Gemini request types
│           └── response.rs         # Gemini response types
├── repository/
│   ├── mod.rs
│   ├── ai_requests/
│   │   ├── mod.rs
│   │   ├── repository.rs           # AiRequestRepository struct
│   │   ├── queries.rs              # Read operations, usage queries
│   │   ├── mutations.rs            # Write operations
│   │   └── message_operations.rs   # Message/tool call persistence
│   └── evaluations/
│       └── mod.rs                  # EvaluationRepository
└── services/
    ├── mod.rs
    ├── config/
    │   ├── mod.rs
    │   └── validator.rs            # ConfigValidator
    ├── core/
    │   ├── mod.rs
    │   ├── image_service.rs        # ImageService
    │   ├── request_logging.rs      # Structured logging
    │   ├── ai_service/
    │   │   ├── mod.rs
    │   │   ├── service.rs          # AiService struct (accepts ToolProvider)
    │   │   ├── generation.rs       # generate()
    │   │   ├── tool_execution.rs   # generate_with_tools()
    │   │   ├── streaming.rs        # generate_stream(), health_check()
    │   │   ├── planning.rs         # generate_plan(), estimate_cost()
    │   │   └── provider_impl.rs    # AiProvider trait implementation
    │   └── request_storage/
    │       ├── mod.rs
    │       ├── storage.rs          # RequestStorage, StoreParams
    │       ├── record_builder.rs   # build_record(), extract_messages()
    │       └── async_operations.rs # Background persistence
    ├── providers/
    │   ├── mod.rs
    │   ├── provider_trait.rs       # AiProvider trait
    │   ├── provider_factory.rs     # ProviderFactory
    │   ├── image_provider_trait.rs # ImageProvider trait
    │   ├── gemini_images.rs        # GeminiImageProvider
    │   ├── shared/
    │   │   ├── mod.rs
    │   │   ├── http_client.rs
    │   │   └── response_builder.rs
    │   ├── anthropic/
    │   │   ├── mod.rs
    │   │   ├── provider.rs
    │   │   ├── converters.rs
    │   │   ├── generation.rs
    │   │   └── trait_impl.rs
    │   ├── openai/
    │   │   ├── mod.rs
    │   │   ├── provider.rs
    │   │   ├── converters.rs
    │   │   ├── generation.rs
    │   │   ├── streaming.rs
    │   │   ├── response_builder.rs
    │   │   └── trait_impl.rs
    │   └── gemini/
    │       ├── mod.rs
    │       ├── constants.rs
    │       ├── provider.rs
    │       ├── converters.rs
    │       ├── request_builders.rs
    │       ├── tool_conversion.rs
    │       ├── tools.rs
    │       ├── generation.rs
    │       ├── streaming.rs
    │       ├── search.rs
    │       ├── code_execution.rs
    │       └── trait_impl.rs
    ├── schema/
    │   ├── mod.rs
    │   ├── analyzer.rs
    │   ├── capabilities.rs
    │   ├── mapper.rs
    │   ├── sanitizer.rs
    │   └── transformer.rs
    ├── storage/
    │   ├── mod.rs
    │   └── image_storage.rs        # ImageStorage, StorageConfig
    ├── structured_output/
    │   ├── mod.rs                  # StructuredOutputProcessor
    │   ├── parser.rs
    │   └── validator.rs
    ├── tooled/
    │   ├── mod.rs
    │   ├── executor.rs             # TooledExecutor, ResponseStrategy
    │   ├── formatter.rs            # ToolResultFormatter
    │   └── synthesizer.rs          # ResponseSynthesizer
    └── tools/
        ├── mod.rs
        ├── adapter.rs              # Type conversions for ToolProvider
        ├── discovery.rs            # ToolDiscovery (uses ToolProvider trait)
        └── noop_provider.rs        # NoopToolProvider for non-tool use
```

## Database

Tables: `ai_requests`, `ai_request_messages`, `ai_request_tool_calls`
