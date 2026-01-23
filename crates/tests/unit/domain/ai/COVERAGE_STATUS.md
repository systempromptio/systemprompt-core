# AI Crate Test Coverage Status

**Last Updated:** 2026-01-22
**Total Tests:** 638

## Module Coverage Summary

| Module | Status | Tests | Notes |
|--------|--------|-------|-------|
| `error` | ✅ Complete | 28 | All error types and conversions tested |
| `extension` | ✅ Complete | 14 | AiExtension metadata, schemas, dependencies |
| `models/ai_request_record` | ✅ Complete | 34 | Builder pattern, status, tokens, cache |
| `models/image_generation` | ✅ Complete | 30 | Resolution, aspect ratio, requests, responses |
| `models/message_converters` | ✅ Complete | 16 | Anthropic, OpenAI, Gemini role mappings |
| `models/providers/anthropic` | ✅ Complete | 27 | Content blocks, messages, requests, responses |
| `models/providers/gemini` | ✅ Complete | 16 | Content, parts, models |
| `models/providers/openai` | ✅ Complete | 20 | Messages, requests, response formats |
| `services/config/validator` | ✅ Complete | 36 | Config validation, MCP, history settings |
| `services/schema/analyzer` | ✅ Complete | 14 | DiscriminatedUnion detection |
| `services/schema/capabilities` | ✅ Complete | 14 | Provider capability checks |
| `services/schema/mapper` | ✅ Complete | 20 | Tool name mapping |
| `services/schema/sanitizer` | ✅ Complete | 18 | Schema cleanup |
| `services/schema/transformer` | ✅ Complete | 22 | Schema transformation |
| `services/storage/image_storage` | ✅ Complete | 16 | File operations, config validation |
| `services/structured_output/parser` | ✅ Complete | 28 | JSON extraction, heuristics |
| `services/structured_output/validator` | ✅ Complete | 40 | Type validation, nested objects |
| `services/tooled/executor` | ✅ Complete | 12 | Response strategies |
| `services/tooled/formatter` | ✅ Complete | 20 | Result formatting |
| `services/tooled/synthesizer` | ✅ Complete | 18 | Fallback generator, prompt builder |
| `services/tools/adapter` | ✅ Complete | 22 | Tool conversion adapters |
| `services/tools/discovery` | ✅ Complete | 13 | Tool discovery with mock provider |
| `services/tools/noop_provider` | ✅ Complete | 8 | NoopToolProvider behavior |
| `services/providers/provider_factory` | ✅ Complete | 14 | Provider creation, create_all |
| `services/providers/image_provider_factory` | ✅ Complete | 14 | Image provider creation |
| `services/providers/image_provider_trait` | ✅ Complete | 22 | Capabilities, GeminiImageProvider |
| `services/providers/provider_trait` | ✅ Complete | 16 | Generation params |
| `services/providers/anthropic/*` | ✅ Complete | 24 | Converters, thinking |
| `services/providers/gemini/*` | ✅ Complete | 24 | Converters, tool conversion |
| `services/providers/openai/*` | ✅ Complete | 20 | Converters, reasoning, response builder |
| `services/providers/shared/*` | ✅ Complete | 8 | Response builder |

## Coverage Gaps

### Repository Layer (Integration Tests Recommended)
- `repository/ai_requests/*` - Database operations require integration tests with real DB
- Queries, mutations, message operations - Better suited for `crates/tests/integration/ai/`

### Core Services (Requires Extensive Mocking)
- `services/core/ai_service/*` - Would need HTTP mocks for provider calls
- `services/core/image_service.rs` - Would need HTTP mocks
- `services/core/request_storage/*` - Requires AiRequest/AiResponse objects

### Provider Implementations (Requires HTTP Mocking)
- `services/providers/*/generation.rs` - HTTP calls to external APIs
- `services/providers/*/streaming.rs` - Streaming HTTP responses
- `services/providers/*/trait_impl.rs` - Full trait implementation tests

## How to Run Tests

```bash
# Run all AI tests
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-ai-tests

# Run with output
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-ai-tests -- --nocapture

# Run specific module
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-ai-tests services::schema::
```

## Coverage Report

To generate a coverage report:

```bash
cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p systemprompt-ai-tests --html
```

## Recent Changes

### 2026-01-22
- Added 90 new tests (548 → 638)
- New test modules:
  - `services/providers/provider_factory.rs` - 14 tests
  - `services/providers/image_provider_factory.rs` - 14 tests
  - `services/providers/image_provider_trait.rs` - 22 tests
  - `services/tooled/synthesizer.rs` - 18 tests
  - `services/tools/discovery.rs` - 13 tests
  - `extension.rs` - 14 tests
