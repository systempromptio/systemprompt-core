//! Tests for provider trait types.

use systemprompt_core_ai::services::providers::{
    GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, ToolResultsParams,
};
use systemprompt_core_ai::models::ai::{AiMessage, MessageRole, ResponseFormat, SamplingParams};
use systemprompt_core_ai::models::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_identifiers::{AiToolCallId, McpServerId};
use serde_json::json;

mod model_pricing_tests {
    use super::*;

    #[test]
    fn new_creates_pricing() {
        let pricing = ModelPricing::new(0.01, 0.03);

        assert!((pricing.input_cost_per_1k - 0.01).abs() < f32::EPSILON);
        assert!((pricing.output_cost_per_1k - 0.03).abs() < f32::EPSILON);
    }

    #[test]
    fn pricing_is_copy() {
        let pricing = ModelPricing::new(0.02, 0.04);
        let copied = pricing;

        assert!((pricing.input_cost_per_1k - copied.input_cost_per_1k).abs() < f32::EPSILON);
    }

    #[test]
    fn pricing_is_debug() {
        let pricing = ModelPricing::new(0.01, 0.02);
        let debug = format!("{:?}", pricing);

        assert!(debug.contains("ModelPricing"));
    }

    #[test]
    fn pricing_is_clone() {
        let pricing = ModelPricing::new(0.05, 0.10);
        let cloned = pricing.clone();

        assert!((pricing.input_cost_per_1k - cloned.input_cost_per_1k).abs() < f32::EPSILON);
    }
}

mod generation_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![
            AiMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                parts: Vec::new(),
            },
        ]
    }

    #[test]
    fn new_creates_params() {
        let messages = sample_messages();
        let params = GenerationParams::new(&messages, "gpt-4", 1024);

        assert_eq!(params.model, "gpt-4");
        assert_eq!(params.max_output_tokens, 1024);
        assert!(params.sampling.is_none());
        assert_eq!(params.messages.len(), 1);
    }

    #[test]
    fn with_sampling_adds_sampling() {
        let messages = sample_messages();
        let sampling = SamplingParams {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: None,
            presence_penalty: None,
            frequency_penalty: None,
            stop_sequences: None,
        };

        let params = GenerationParams::new(&messages, "gpt-4", 1024).with_sampling(&sampling);

        assert!(params.sampling.is_some());
        assert_eq!(params.sampling.unwrap().temperature, Some(0.7));
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let params = GenerationParams::new(&messages, "model", 512);
        let cloned = params.clone();

        assert_eq!(params.model, cloned.model);
        assert_eq!(params.max_output_tokens, cloned.max_output_tokens);
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let params = GenerationParams::new(&messages, "test-model", 256);
        let debug = format!("{:?}", params);

        assert!(debug.contains("GenerationParams"));
        assert!(debug.contains("test-model"));
    }
}

mod tool_generation_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![AiMessage {
            role: MessageRole::User,
            content: "Use a tool".to_string(),
            parts: Vec::new(),
        }]
    }

    fn sample_tools() -> Vec<McpTool> {
        vec![McpTool {
            name: "calculator".to_string(),
            description: Some("Performs math".to_string()),
            input_schema: Some(json!({"type": "object"})),
            output_schema: None,
            service_id: McpServerId::new("math-service"),
            terminal_on_success: false,
            model_config: None,
        }]
    }

    #[test]
    fn new_creates_params() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "gpt-4", 1024);
        let tools = sample_tools();

        let params = ToolGenerationParams::new(base, tools);

        assert_eq!(params.base.model, "gpt-4");
        assert_eq!(params.tools.len(), 1);
        assert_eq!(params.tools[0].name, "calculator");
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "gpt-4", 1024);
        let tools = sample_tools();

        let params = ToolGenerationParams::new(base, tools);
        let cloned = params.clone();

        assert_eq!(params.tools.len(), cloned.tools.len());
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 512);
        let tools = sample_tools();

        let params = ToolGenerationParams::new(base, tools);
        let debug = format!("{:?}", params);

        assert!(debug.contains("ToolGenerationParams"));
    }
}

mod tool_results_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![AiMessage {
            role: MessageRole::User,
            content: "Process results".to_string(),
            parts: Vec::new(),
        }]
    }

    fn sample_tool_calls() -> Vec<ToolCall> {
        vec![ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-1"),
            name: "search".to_string(),
            arguments: json!({"query": "test"}),
        }]
    }

    fn sample_results() -> Vec<CallToolResult> {
        vec![CallToolResult {
            content: vec![],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }]
    }

    #[test]
    fn new_creates_params() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "claude-3", 2048);
        let tool_calls = sample_tool_calls();
        let results = sample_results();

        let params = ToolResultsParams::new(base, &tool_calls, &results);

        assert_eq!(params.base.model, "claude-3");
        assert_eq!(params.tool_calls.len(), 1);
        assert_eq!(params.tool_results.len(), 1);
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);
        let tool_calls = sample_tool_calls();
        let results = sample_results();

        let params = ToolResultsParams::new(base, &tool_calls, &results);
        let cloned = params.clone();

        assert_eq!(params.tool_calls.len(), cloned.tool_calls.len());
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);
        let tool_calls = sample_tool_calls();
        let results = sample_results();

        let params = ToolResultsParams::new(base, &tool_calls, &results);
        let debug = format!("{:?}", params);

        assert!(debug.contains("ToolResultsParams"));
    }
}

mod schema_generation_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![AiMessage {
            role: MessageRole::User,
            content: "Generate structured".to_string(),
            parts: Vec::new(),
        }]
    }

    #[test]
    fn new_creates_params() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "gemini-pro", 4096);
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let params = SchemaGenerationParams::new(base, schema);

        assert_eq!(params.base.model, "gemini-pro");
        assert_eq!(params.response_schema["type"], "object");
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);
        let schema = json!({"type": "string"});

        let params = SchemaGenerationParams::new(base, schema);
        let cloned = params.clone();

        assert_eq!(params.response_schema, cloned.response_schema);
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);
        let schema = json!({});

        let params = SchemaGenerationParams::new(base, schema);
        let debug = format!("{:?}", params);

        assert!(debug.contains("SchemaGenerationParams"));
    }
}

mod structured_generation_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![AiMessage {
            role: MessageRole::User,
            content: "Get JSON".to_string(),
            parts: Vec::new(),
        }]
    }

    #[test]
    fn new_creates_params() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "gpt-4", 1024);
        let format = ResponseFormat::JsonObject;

        let params = StructuredGenerationParams::new(base, &format);

        assert_eq!(params.base.model, "gpt-4");
        assert!(matches!(params.response_format, ResponseFormat::JsonObject));
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 512);
        let format = ResponseFormat::Text;

        let params = StructuredGenerationParams::new(base, &format);
        let cloned = params.clone();

        assert_eq!(params.base.model, cloned.base.model);
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 512);
        let format = ResponseFormat::Text;

        let params = StructuredGenerationParams::new(base, &format);
        let debug = format!("{:?}", params);

        assert!(debug.contains("StructuredGenerationParams"));
    }
}

mod search_generation_params_tests {
    use super::*;

    fn sample_messages() -> Vec<AiMessage> {
        vec![AiMessage {
            role: MessageRole::User,
            content: "Search something".to_string(),
            parts: Vec::new(),
        }]
    }

    #[test]
    fn new_creates_minimal_params() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "gemini-flash", 2048);

        let params = SearchGenerationParams::new(base);

        assert_eq!(params.base.model, "gemini-flash");
        assert!(params.urls.is_none());
        assert!(params.response_schema.is_none());
    }

    #[test]
    fn with_urls_adds_urls() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);

        let params = SearchGenerationParams::new(base)
            .with_urls(vec!["https://example.com".to_string()]);

        assert!(params.urls.is_some());
        assert_eq!(params.urls.unwrap()[0], "https://example.com");
    }

    #[test]
    fn with_response_schema_adds_schema() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);
        let schema = json!({"type": "object"});

        let params = SearchGenerationParams::new(base).with_response_schema(schema);

        assert!(params.response_schema.is_some());
        assert_eq!(params.response_schema.unwrap()["type"], "object");
    }

    #[test]
    fn chained_methods() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);

        let params = SearchGenerationParams::new(base)
            .with_urls(vec!["https://test.com".to_string()])
            .with_response_schema(json!({"type": "array"}));

        assert!(params.urls.is_some());
        assert!(params.response_schema.is_some());
    }

    #[test]
    fn params_is_clone() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);

        let params = SearchGenerationParams::new(base)
            .with_urls(vec!["https://example.com".to_string()]);
        let cloned = params.clone();

        assert_eq!(params.urls, cloned.urls);
    }

    #[test]
    fn params_is_debug() {
        let messages = sample_messages();
        let base = GenerationParams::new(&messages, "model", 1024);

        let params = SearchGenerationParams::new(base);
        let debug = format!("{:?}", params);

        assert!(debug.contains("SearchGenerationParams"));
    }
}
