# Changelog

## [0.63.0] - 2026-10-07

### Added

- First release. The provider wire codecs (`canonical`, `anthropic`, `openai_chat`, `openai_responses`, `gemini`, `upstream`, `inspect`, `sse`, `defect`, `error`) move here from `systemprompt_models::wire`, the JSON-Schema capability matrices and sanitiser from `systemprompt_models::schema`, `WireProtocol`, `Hosting` and `ModelLimits` from `systemprompt_models::services`, and `is_vertex_host`, `names_a_project_literally`, `PROJECT_PLACEHOLDER` and `REGION_PLACEHOLDER` (now `hosting::*`) from `systemprompt_models::services::providers`. The crate depends only on `systemprompt-identifiers`. Migrate by depending on `systemprompt-wire` and renaming `systemprompt_models::wire::*` to `systemprompt_wire::*`.
- `gemini` is the one public definition of the `generateContent` body (`GeminiRequest`, `GeminiContent`, `GeminiPart`, `GeminiInlineData`, `GeminiGenerationConfig`, `GeminiImageConfig`, `GeminiTool`, `GeminiEmpty`, `GeminiResponse`, `GeminiCandidate`, …), shared by the gateway and the AI image provider. `GeminiTool::GoogleSearch { google_search: GeminiEmpty {} }` replaces the AI crate's `GoogleSearch`, `GeminiPart::Text` carries `thought` and `thought_signature`, and `GeminiGenerationConfig` gains `response_modalities` and `image_config`.
- `CanonicalContent::AnthropicToolBlock { tool_name, block }` and `CanonicalTool::anthropic_definition`: an Anthropic server-tool block or native tool definition is carried verbatim to an Anthropic upstream and rendered as text for the other dialects.

### Changed

- Relative to `systemprompt_models::wire` in 0.62: `WireProtocol::surface` is removed in favour of `systemprompt_manifest::services::providers::surface_for(protocol)`; `origin`, `BUFFERED_BODY_LIMIT_BYTES` and `gateway_hash` stay outside this crate (`systemprompt_models::origin`, `systemprompt_models::net::BUFFERED_BODY_LIMIT_BYTES`, `systemprompt_identifiers::gateway_hash`).
