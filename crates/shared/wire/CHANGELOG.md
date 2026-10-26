# Changelog

## [0.73.0] - 2026-10-25

### Breaking

- **Breaking:** `CanonicalRequest` groups its sampling, tooling and reasoning fields into `sampling: CanonicalSampling`, `tooling: CanonicalTooling` and `reasoning: CanonicalReasoning`. Migrate by reading and building the fields through those groups.

### Changed

- Header constants come from `systemprompt_identifiers::headers`.

## [0.72.0] - 2026-10-22

### Added

- `canonical::normalize_turns`, `TurnMerge` and `INTERRUPTED_TOOL_RESULT`: one pass, run before every upstream encoder, that merges consecutive same-side turns, orders tool results first, answers an unanswered `tool_use` with a synthetic error result and drops an orphan tool result.
- `anthropic::upstream_messages`, `anthropic::history_is_strict` and `openai_chat::history_is_strict`.

### Fixed

- Tool results that follow an Anthropic server-tool turn are kept.

## [0.71.0] - 2026-10-21

### Changed

- `x-opencode-*` headers are relayed upstream to a provider whose `forward_headers` lists them.

## [0.66.0] - 2026-10-13

### Changed

- Routes match server tools by type; Vertex bodies reconcile the tools Vertex lacks; audit rows name their route.

### Fixed

- Clippy findings in the wire codecs.

## [0.63.0] - 2026-10-07

### Added

- First release. The provider wire codecs (`canonical`, `anthropic`, `openai_chat`, `openai_responses`, `gemini`, `upstream`, `inspect`, `sse`, `defect`, `error`) move here from `systemprompt_models::wire`, the JSON-Schema capability matrices and sanitiser from `systemprompt_models::schema`, `WireProtocol`, `Hosting` and `ModelLimits` from `systemprompt_models::services`, and `is_vertex_host`, `names_a_project_literally`, `PROJECT_PLACEHOLDER` and `REGION_PLACEHOLDER` (now `hosting::*`) from `systemprompt_models::services::providers`. The crate depends only on `systemprompt-identifiers`. Migrate by depending on `systemprompt-wire` and renaming `systemprompt_models::wire::*` to `systemprompt_wire::*`.
- `gemini` is the one public definition of the `generateContent` body (`GeminiRequest`, `GeminiContent`, `GeminiPart`, `GeminiInlineData`, `GeminiGenerationConfig`, `GeminiImageConfig`, `GeminiTool`, `GeminiEmpty`, `GeminiResponse`, `GeminiCandidate`, …), shared by the gateway and the AI image provider. `GeminiTool::GoogleSearch { google_search: GeminiEmpty {} }` replaces the AI crate's `GoogleSearch`, `GeminiPart::Text` carries `thought` and `thought_signature`, and `GeminiGenerationConfig` gains `response_modalities` and `image_config`.
- `CanonicalContent::AnthropicToolBlock { tool_name, block }` and `CanonicalTool::anthropic_definition`: an Anthropic server-tool block or native tool definition is carried verbatim to an Anthropic upstream and rendered as text for the other dialects.

### Changed

- Relative to `systemprompt_models::wire` in 0.62: `WireProtocol::surface` is removed in favour of `systemprompt_manifest::services::providers::surface_for(protocol)`; `origin`, `BUFFERED_BODY_LIMIT_BYTES` and `gateway_hash` stay outside this crate (`systemprompt_models::origin`, `systemprompt_models::net::BUFFERED_BODY_LIMIT_BYTES`, `systemprompt_identifiers::gateway_hash`).
