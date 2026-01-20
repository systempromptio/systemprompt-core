# AI Generation System

This document explains how AI generation is configured and executed in systemprompt-core, from YAML configuration through to provider execution.

---

## Configuration Hierarchy

The AI configuration follows a strict 3-level override hierarchy:

```
┌─────────────────────────────────────────────────────────────────┐
│ Level 1: Global (AiConfig from ai.yaml)                         │
│ - default_provider, default_model, default_max_output_tokens    │
│ - Applies to all requests unless overridden                     │
└────────────────────┬────────────────────────────────────────────┘
                     │ Can be overridden by:
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│ Level 2: Agent (AgentMetadataConfig from agents.yaml)           │
│ - provider, model, max_output_tokens                            │
│ - Applies to all requests from this agent                       │
│ - tool_model_overrides: per-tool configuration                  │
└────────────────────┬────────────────────────────────────────────┘
                     │ Can be overridden by:
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│ Level 3: Tool (ToolModelConfig via RequestContext)              │
│ - provider, model, max_output_tokens for specific tool calls    │
│ - Set dynamically when tool execution enriches context          │
└─────────────────────────────────────────────────────────────────┘
```

**Priority**: Tool > Agent > Global (highest to lowest)

---

## YAML Configuration Files

### 1. AI Configuration (`ai.yaml`)

Defines global AI settings and provider configurations.

```yaml
# ai.yaml
default_provider: anthropic
default_model: claude-3-5-sonnet-20241022
default_max_output_tokens: 8192

sampling:
  temperature: 0.7
  top_p: 0.9

providers:
  anthropic:
    enabled: true
    api_key: ${ANTHROPIC_API_KEY}
    default_model: claude-3-5-sonnet-20241022
    models:
      claude-3-5-sonnet-20241022:
        max_tokens: 8192
      claude-3-opus-20240229:
        max_tokens: 4096

  openai:
    enabled: true
    api_key: ${OPENAI_API_KEY}
    default_model: gpt-4-turbo
    models:
      gpt-4-turbo:
        max_tokens: 4096
      gpt-4o:
        max_tokens: 16384

  gemini:
    enabled: true
    api_key: ${GEMINI_API_KEY}
    default_model: gemini-2.5-flash
    google_search_enabled: false
```

**Key Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `default_provider` | String | Provider to use when none specified |
| `default_model` | String | Model to use when none specified |
| `default_max_output_tokens` | u32 | Max tokens when none specified (default: 8192) |
| `sampling` | Object | Default sampling parameters |
| `providers` | Map | Provider-specific configurations |

### 2. Agent Configuration (`agents.yaml`)

Defines agents with optional AI overrides.

```yaml
# agents.yaml
agents:
  - name: general-assistant
    port: 9000
    enabled: true
    is_primary: true
    metadata:
      system_prompt: "You are a helpful assistant."
      mcp_servers:
        - content-service
      # Optional AI overrides (Level 2)
      provider: anthropic
      model: claude-3-5-sonnet-20241022
      max_output_tokens: 16000

      # Per-tool overrides (Level 3)
      tool_model_overrides:
        content-service:
          generate_content:
            provider: anthropic
            model: claude-3-opus-20240229
            max_output_tokens: 32000
          summarize:
            provider: openai
            model: gpt-4o
            max_output_tokens: 4000

  - name: code-assistant
    port: 9001
    enabled: true
    metadata:
      system_prompt: "You are a coding expert."
      provider: openai
      model: gpt-4o
      max_output_tokens: 8000
```

**Agent Metadata Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `system_prompt` | String | No | Agent's system prompt |
| `mcp_servers` | [String] | No | MCP servers this agent can use |
| `provider` | String | No | Override default provider |
| `model` | String | No | Override default model |
| `max_output_tokens` | u32 | No | Override default max tokens |
| `tool_model_overrides` | Map | No | Per-service, per-tool overrides |

### 3. Tool Model Overrides Structure

```yaml
tool_model_overrides:
  <service-name>:
    <tool-name>:
      provider: <provider>      # Optional
      model: <model>            # Optional
      max_output_tokens: <u32>  # Optional
```

Each field is optional - only specify what you want to override.

---

## Rust Structs

### Configuration Structs

**AiConfig** (`crates/shared/models/src/services/ai.rs`)
```rust
pub struct AiConfig {
    pub default_provider: String,
    pub default_max_output_tokens: Option<u32>,
    pub sampling: SamplingConfig,
    pub providers: HashMap<String, AiProviderConfig>,
    pub tool_models: HashMap<String, ToolModelSettings>,
    pub mcp: McpConfig,
    pub history: HistoryConfig,
}
```

**AgentMetadataConfig** (`crates/shared/models/src/services/agent_config.rs`)
```rust
pub struct AgentMetadataConfig {
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub tool_model_overrides: ToolModelOverrides,
}
```

**ToolModelConfig** (`crates/shared/models/src/ai/models.rs`)
```rust
pub struct ToolModelConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_output_tokens: Option<u32>,
}

pub type ToolModelOverrides = HashMap<String, HashMap<String, ToolModelConfig>>;
```

### Runtime Structs

**AgentRuntimeInfo** (`crates/domain/agent/src/models/runtime.rs`)
```rust
pub struct AgentRuntimeInfo {
    pub name: String,
    pub port: u16,
    pub is_enabled: bool,
    pub is_primary: bool,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub skills: Vec<String>,
    pub tool_model_overrides: ToolModelOverrides,
}
```

### Execution Structs (Canonical)

**GenerationParams** (`crates/domain/ai/src/services/providers/provider_trait.rs`)

This is the **single canonical struct** used for all AI generation calls:

```rust
pub struct GenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub model: &'a str,
    pub max_output_tokens: u32,
    pub sampling: Option<&'a SamplingParams>,
}
```

**ToolGenerationParams** - extends GenerationParams with tools:
```rust
pub struct ToolGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    pub tools: Vec<McpTool>,
}
```

**SchemaGenerationParams** - extends GenerationParams with JSON schema:
```rust
pub struct SchemaGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    pub response_schema: serde_json::Value,
}
```

---

## Configuration Resolution

### Resolution Function

The configuration hierarchy is resolved at request time:

**Location**: `crates/domain/agent/src/services/a2a_server/processing/ai_executor.rs`

```rust
fn resolve_provider_config(
    request_context: &RequestContext,
    agent_runtime: &AgentRuntimeInfo,
    ai_service: &dyn AiProvider,
) -> (String, String, u32) {
    // Priority 1: Tool-level config (highest)
    if let Some(config) = request_context.tool_model_config() {
        let provider = config.provider.as_deref()
            .or(agent_runtime.provider.as_deref())
            .unwrap_or_else(|| ai_service.default_provider());
        let model = config.model.as_deref()
            .or(agent_runtime.model.as_deref())
            .unwrap_or_else(|| ai_service.default_model());
        let max_tokens = config.max_output_tokens
            .or(agent_runtime.max_output_tokens)
            .unwrap_or_else(|| ai_service.default_max_output_tokens());
        return (provider, model, max_tokens);
    }

    // Priority 2: Agent-level config
    let provider = agent_runtime.provider.as_deref()
        .unwrap_or_else(|| ai_service.default_provider());
    let model = agent_runtime.model.as_deref()
        .unwrap_or_else(|| ai_service.default_model());

    // Priority 3: Global defaults (lowest)
    let max_tokens = agent_runtime.max_output_tokens
        .unwrap_or_else(|| ai_service.default_max_output_tokens());

    (provider, model, max_tokens)
}
```

### When Tool Config is Applied

Tool-level configuration is injected into `RequestContext` when:
1. A tool execution is triggered
2. The agent has `tool_model_overrides` configured for that tool
3. The override is applied via `request_context.with_tool_model_config()`

---

## Complete Data Flow

```
                            CONFIGURATION PHASE
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  ai.yaml ──────► AiConfig ──────► AiService                         │
│                   │                 │                               │
│                   │                 ├─ default_provider: "anthropic"│
│                   │                 ├─ default_model: "claude-3..."  │
│                   │                 └─ default_max_output_tokens: 8192│
│                   │                                                 │
│  agents.yaml ──► AgentConfig ──► AgentRuntimeInfo                   │
│                   │                 │                               │
│                   │                 ├─ provider: Some("openai")     │
│                   │                 ├─ model: Some("gpt-4o")        │
│                   │                 ├─ max_output_tokens: Some(16000)│
│                   │                 └─ tool_model_overrides: {...}  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

                            REQUEST PHASE
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  User Message                                                       │
│       │                                                             │
│       ▼                                                             │
│  RequestContext created                                             │
│       │                                                             │
│       ├─► If tool execution: inject ToolModelConfig                 │
│       │                                                             │
│       ▼                                                             │
│  resolve_provider_config()                                          │
│       │                                                             │
│       ├─ Check tool_model_config (Priority 1)                       │
│       ├─ Check agent_runtime (Priority 2)                           │
│       └─ Fall back to ai_service defaults (Priority 3)              │
│       │                                                             │
│       ▼                                                             │
│  Resolved: (provider, model, max_output_tokens)                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

                            EXECUTION PHASE
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  Build AiRequest                                                    │
│       │                                                             │
│       ▼                                                             │
│  AiRequest::builder(messages, provider, model, max_tokens, context) │
│       │                                                             │
│       ▼                                                             │
│  Select provider: providers.get(provider)                           │
│       │                                                             │
│       ▼                                                             │
│  Create GenerationParams {                                          │
│      messages: &request.messages,                                   │
│      model: resolved_model,                                         │
│      max_output_tokens: resolved_tokens,                            │
│      sampling: request.sampling,                                    │
│  }                                                                  │
│       │                                                             │
│       ▼                                                             │
│  provider.generate(params)  ◄─── DIRECT PASSTHROUGH                 │
│       │                                                             │
│       ▼                                                             │
│  Provider-specific API call (OpenAI/Anthropic/Gemini)               │
│       │                                                             │
│       ▼                                                             │
│  AiResponse                                                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Provider Implementations

All providers use the same canonical structs with direct passthrough:

### OpenAI
```rust
// crates/domain/ai/src/services/providers/openai/trait_impl.rs
async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
    generation::generate(self, params).await  // Direct passthrough
}

async fn generate_with_tools(&self, params: ToolGenerationParams<'_>) -> Result<...> {
    generation::generate_with_tools(self, params).await  // Direct passthrough
}
```

### Anthropic
```rust
// crates/domain/ai/src/services/providers/anthropic/trait_impl.rs
async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
    generation::generate(self, params).await  // Direct passthrough
}
```

### Gemini
```rust
// crates/domain/ai/src/services/providers/gemini/trait_impl.rs
async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
    generation::generate(self, params).await  // Direct passthrough
}
```

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `crates/shared/models/src/services/ai.rs` | AiConfig struct |
| `crates/shared/models/src/services/agent_config.rs` | AgentMetadataConfig struct |
| `crates/shared/models/src/ai/models.rs` | ToolModelConfig struct |
| `crates/shared/models/src/ai/sampling.rs` | SamplingParams struct |
| `crates/domain/agent/src/models/runtime.rs` | AgentRuntimeInfo struct |
| `crates/domain/ai/src/services/providers/provider_trait.rs` | GenerationParams (canonical) |
| `crates/domain/agent/src/services/a2a_server/processing/ai_executor.rs` | resolve_provider_config() |
| `crates/domain/agent/src/services/a2a_server/processing/strategies/planned.rs` | build_ai_request() |
| `crates/domain/ai/src/services/core/ai_service/planning.rs` | generate_response() |

---

## Configuration Examples

### Example 1: Global Defaults Only

```yaml
# ai.yaml
default_provider: anthropic
default_model: claude-3-5-sonnet-20241022
default_max_output_tokens: 8192

# agents.yaml
agents:
  - name: my-agent
    port: 9000
    metadata:
      system_prompt: "You are helpful."
      # No provider/model/max_output_tokens = uses ai.yaml defaults
```

**Result**: All requests use `anthropic`, `claude-3-5-sonnet-20241022`, `8192 tokens`

### Example 2: Agent Override

```yaml
# ai.yaml
default_provider: anthropic
default_model: claude-3-5-sonnet-20241022
default_max_output_tokens: 8192

# agents.yaml
agents:
  - name: code-agent
    metadata:
      provider: openai
      model: gpt-4o
      max_output_tokens: 16000
```

**Result**: Requests from `code-agent` use `openai`, `gpt-4o`, `16000 tokens`

### Example 3: Tool-Specific Override

```yaml
# agents.yaml
agents:
  - name: content-agent
    metadata:
      provider: anthropic
      model: claude-3-5-sonnet-20241022
      max_output_tokens: 8192
      tool_model_overrides:
        content-service:
          generate_long_content:
            model: claude-3-opus-20240229
            max_output_tokens: 32000
```

**Result**:
- Normal requests: `anthropic`, `claude-3-5-sonnet-20241022`, `8192`
- When `generate_long_content` tool runs: `anthropic`, `claude-3-opus-20240229`, `32000`

---

## Debugging

### Check Resolved Config

Look for these log messages:

```
Using tool_model_config in planned strategy
  provider=anthropic
  model=claude-3-opus-20240229
  max_output_tokens=32000
```

### CLI Commands

```bash
# View recent AI requests with full details
systemprompt infra logs request list --since 1h
systemprompt infra logs request show <REQUEST_ID> --messages --full
```

### Verify Config Loading

```bash
# Check if agent config is loaded correctly
systemprompt admin agents list --verbose
```
