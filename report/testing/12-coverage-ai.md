# Domain: AI Crate Coverage

## Current State

**Source code:** 98 source files in `crates/domain/ai/src/` (~7,500+ lines)
**Test code:** 50 test files in `crates/tests/unit/domain/ai/src/` (637 tests -- 628 sync, 9 async)
**Coverage:** 42% of source files have corresponding tests, but tests are shallow

### What IS Tested

- **Models:** 6 test files (serialization, validation)
- **Services:** 9 test files covering:
  - Schema transformation and validation (`capabilities.rs` -- recently fixed)
  - Tool execution basics
  - Config and structured output
  - Provider model lists
  - Error types

### What is NOT Tested

| Area | Files | Gap Description |
|------|-------|-----------------|
| Provider implementations | 15+ files (Anthropic, Gemini, OpenAI) | ZERO tests. No tests for API call construction, response parsing, streaming chunk handling, token counting, or rate limiting/retry logic. |
| Generation logic | `core/ai_service/generation.rs` | ZERO tests. The core generation pipeline is completely uncovered. |
| Request storage | Async persistence layer | ZERO tests. |
| Streaming responses | Streaming chunk assembly and delivery | ZERO tests. |
| Image persistence | Image storage and retrieval | ZERO tests. |

### Test Quality Assessment

The 637 tests are mostly schema validation and model boilerplate -- verifying that structs serialize and deserialize correctly. Actual AI service behavior (calling providers, processing responses, handling failures) is completely untested. The high test count creates a false sense of security.

### Risk Assessment

LLM provider integrations are complex, error-prone, and change frequently. Without tests, provider API changes (new response formats, deprecated fields, changed error codes) will cause silent failures in production. Streaming response handling is particularly fragile and entirely uncovered.

---

## Desired State

- Every provider implementation (Anthropic, Gemini, OpenAI) has unit tests covering: request construction, response parsing, streaming chunk handling, error mapping, and retry behavior
- The generation pipeline has tests for: prompt assembly, provider selection, response post-processing, and failure recovery
- Streaming response handling has tests for: chunk ordering, partial message assembly, connection drops, and timeout behavior
- Token counting has tests for: accuracy against known inputs, edge cases (empty input, max context), and provider-specific counting differences
- Request storage has tests for: persistence correctness, retrieval, and cleanup
- Target: 70%+ source file coverage with tests that exercise actual behavior, not just serialization

---

## How to Get There

### Phase 1: Provider Unit Tests (Highest Impact)

1. Create a shared HTTP mock infrastructure for provider tests (mock server or recorded responses)
2. For each provider (Anthropic, Gemini, OpenAI), test:
   - Request construction: correct headers, body format, model selection
   - Response parsing: successful responses, partial responses, malformed responses
   - Error handling: rate limits (429), auth errors (401), server errors (500), timeout
   - Streaming: chunk parsing, SSE event handling, stream completion detection
3. Use recorded API responses as test fixtures to avoid flaky external calls

### Phase 2: Generation Pipeline Tests

1. Test `generation.rs`: prompt assembly from different input types
2. Test provider selection logic: fallback behavior, model availability
3. Test response post-processing: content extraction, metadata attachment
4. Test failure recovery: retry on transient errors, graceful degradation

### Phase 3: Streaming Infrastructure Tests

1. Test streaming chunk assembly: correct ordering, duplicate detection
2. Test connection lifecycle: establishment, keepalive, clean shutdown, unexpected disconnect
3. Test backpressure: slow consumer handling, buffer limits

### Phase 4: Supporting Services

1. Test token counting against known inputs for each provider
2. Test request storage: write, read, cleanup lifecycle
3. Test image persistence: upload, retrieval, format handling

---

## Incremental Improvement Strategy

### Week 1-2: HTTP Mock Infrastructure + Anthropic Provider
- Build reusable HTTP mock infrastructure (recorded response fixtures, mock server helpers)
- Write tests for the Anthropic provider: request construction, response parsing, error handling
- Target: 25 new behavioral tests, replacing false confidence from serialization-only tests

### Week 3-4: Gemini and OpenAI Providers
- Write equivalent provider tests for Gemini and OpenAI
- Test provider-specific quirks (different streaming formats, error codes, rate limit headers)
- Target: 30 new tests across both providers

### Week 5-6: Generation Pipeline and Streaming
- Write tests for `generation.rs` covering the full prompt-to-response pipeline
- Write streaming tests for chunk assembly and connection handling
- Target: 20 new tests

### Week 7-8: Token Counting, Storage, and Edge Cases
- Add token counting tests with known inputs
- Add request storage and image persistence tests
- Add edge case tests: empty prompts, maximum context length, concurrent requests
- Target: 20 new tests

### Ongoing
- Every provider API change must be accompanied by updated test fixtures
- New provider integrations must ship with full test coverage (request, response, streaming, errors)
- Monthly audit: compare test coverage against production error logs to identify untested failure modes
