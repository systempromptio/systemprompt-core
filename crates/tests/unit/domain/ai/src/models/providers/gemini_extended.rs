use serde_json::json;
use systemprompt_ai::models::providers::gemini::{
    CodeExecution, GeminiCandidate, GeminiCodeExecutionResult, GeminiContent,
    GeminiFunctionCall, GeminiFunctionCallingConfig, GeminiFunctionCallingMode,
    GeminiFunctionDeclaration, GeminiFunctionResponse, GeminiGenerationConfig,
    GeminiGroundingChunk, GeminiGroundingMetadata, GeminiGroundingSupport, GeminiImageConfig,
    GeminiInlineData, GeminiPart, GeminiRequest, GeminiResponse, GeminiSafetyRating,
    GeminiSafetySetting, GeminiTextSegment, GeminiThinkingConfig, GeminiTool, GeminiToolConfig,
    GeminiUrlContextMetadata, GeminiUrlMetadata, GeminiUsageMetadata, GeminiWebSource,
    GoogleSearch, UrlContext,
};

mod gemini_part_variants {
    use super::*;

    #[test]
    fn inline_data_roundtrip() {
        let part = GeminiPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: "image/png".to_owned(),
                data: "base64encodeddata".to_owned(),
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        assert!(json.contains("inlineData"));
        assert!(json.contains("image/png"));
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "image/png");
                assert_eq!(inline_data.data, "base64encodeddata");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn function_call_roundtrip() {
        let part = GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: "my_tool".to_owned(),
                args: json!({"key": "value"}),
                thought_signature: None,
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        assert!(json.contains("functionCall"));
        assert!(json.contains("my_tool"));
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "my_tool");
                assert_eq!(function_call.args["key"], "value");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn function_call_with_thought_signature() {
        let part = GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: "tool_with_thought".to_owned(),
                args: json!({}),
                thought_signature: Some("sig_abc123".to_owned()),
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        assert!(json.contains("thoughtSignature"));
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::FunctionCall { function_call } => {
                assert_eq!(function_call.thought_signature.as_deref(), Some("sig_abc123"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn function_response_roundtrip() {
        let part = GeminiPart::FunctionResponse {
            function_response: GeminiFunctionResponse {
                name: "my_tool".to_owned(),
                response: json!({"result": 42}),
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        assert!(json.contains("functionResponse"));
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::FunctionResponse { function_response } => {
                assert_eq!(function_response.name, "my_tool");
                assert_eq!(function_response.response["result"], 42);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn code_execution_result_roundtrip() {
        let part = GeminiPart::CodeExecutionResult {
            code_execution_result: GeminiCodeExecutionResult {
                outcome: "OUTCOME_OK".to_owned(),
                output: Some("hello world\n".to_owned()),
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        assert!(json.contains("codeExecutionResult"));
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::CodeExecutionResult { code_execution_result } => {
                assert_eq!(code_execution_result.outcome, "OUTCOME_OK");
                assert_eq!(code_execution_result.output.as_deref(), Some("hello world\n"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn code_execution_result_failure() {
        let part = GeminiPart::CodeExecutionResult {
            code_execution_result: GeminiCodeExecutionResult {
                outcome: "OUTCOME_FAILED".to_owned(),
                output: None,
            },
        };
        let json = serde_json::to_string(&part).expect("ser");
        let back: GeminiPart = serde_json::from_str(&json).expect("de");
        match back {
            GeminiPart::CodeExecutionResult { code_execution_result } => {
                assert_eq!(code_execution_result.outcome, "OUTCOME_FAILED");
                assert!(code_execution_result.output.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }
}

mod gemini_generation_config_tests {
    use super::*;

    #[test]
    fn minimal_config_roundtrip() {
        let cfg = GeminiGenerationConfig {
            temperature: None,
            top_p: None,
            top_k: None,
            max_output_tokens: Some(1024),
            stop_sequences: None,
            response_mime_type: None,
            response_schema: None,
            response_modalities: None,
            image_config: None,
            thinking_config: None,
        };
        let json = serde_json::to_string(&cfg).expect("ser");
        assert!(json.contains("maxOutputTokens"));
        assert!(!json.contains("temperature"));
        let back: GeminiGenerationConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(back.max_output_tokens, Some(1024));
        assert!(back.temperature.is_none());
    }

    #[test]
    fn full_config_roundtrip() {
        let cfg = GeminiGenerationConfig {
            temperature: Some(0.7),
            top_p: Some(0.95),
            top_k: Some(40),
            max_output_tokens: Some(2048),
            stop_sequences: Some(vec!["END".to_owned()]),
            response_mime_type: Some("application/json".to_owned()),
            response_schema: Some(json!({"type": "object"})),
            response_modalities: Some(vec!["TEXT".to_owned()]),
            image_config: Some(GeminiImageConfig {
                aspect_ratio: "1:1".to_owned(),
                image_size: Some("1K".to_owned()),
            }),
            thinking_config: Some(GeminiThinkingConfig {
                thinking_budget: Some(8192),
                include_thoughts: Some(true),
            }),
        };
        let json = serde_json::to_string(&cfg).expect("ser");
        let back: GeminiGenerationConfig = serde_json::from_str(&json).expect("de");
        assert!((back.temperature.unwrap() - 0.7).abs() < 1e-6);
        assert!((back.top_p.unwrap() - 0.95).abs() < 1e-6);
        assert_eq!(back.top_k, Some(40));
        assert_eq!(back.max_output_tokens, Some(2048));
        assert_eq!(back.response_mime_type.as_deref(), Some("application/json"));
    }

    #[test]
    fn thinking_config_skip_when_none() {
        let cfg = GeminiGenerationConfig {
            temperature: None,
            top_p: None,
            top_k: None,
            max_output_tokens: Some(512),
            stop_sequences: None,
            response_mime_type: None,
            response_schema: None,
            response_modalities: None,
            image_config: None,
            thinking_config: None,
        };
        let json = serde_json::to_string(&cfg).expect("ser");
        assert!(!json.contains("thinkingConfig"));
    }

    #[test]
    fn thinking_config_serializes() {
        let tc = GeminiThinkingConfig {
            thinking_budget: Some(4096),
            include_thoughts: Some(false),
        };
        let json = serde_json::to_string(&tc).expect("ser");
        assert!(json.contains("thinkingBudget"));
        let back: GeminiThinkingConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(back.thinking_budget, Some(4096));
        assert_eq!(back.include_thoughts, Some(false));
    }

    #[test]
    fn image_config_roundtrip() {
        let ic = GeminiImageConfig {
            aspect_ratio: "16:9".to_owned(),
            image_size: None,
        };
        let json = serde_json::to_string(&ic).expect("ser");
        assert!(!json.contains("imageSize"));
        let back: GeminiImageConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(back.aspect_ratio, "16:9");
        assert!(back.image_size.is_none());
    }
}

mod gemini_tool_config_tests {
    use super::*;

    #[test]
    fn tool_config_auto_mode_roundtrip() {
        let tc = GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode: GeminiFunctionCallingMode::Auto,
                allowed_function_names: None,
            },
        };
        let json = serde_json::to_string(&tc).expect("ser");
        assert!(json.contains("AUTO"));
        assert!(!json.contains("allowedFunctionNames"));
        let back: GeminiToolConfig = serde_json::from_str(&json).expect("de");
        matches!(back.function_calling_config.mode, GeminiFunctionCallingMode::Auto);
    }

    #[test]
    fn tool_config_any_mode_with_names() {
        let tc = GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode: GeminiFunctionCallingMode::Any,
                allowed_function_names: Some(vec!["tool_a".to_owned(), "tool_b".to_owned()]),
            },
        };
        let json = serde_json::to_string(&tc).expect("ser");
        assert!(json.contains("ANY"));
        assert!(json.contains("allowedFunctionNames"));
        let back: GeminiToolConfig = serde_json::from_str(&json).expect("de");
        let names = back.function_calling_config.allowed_function_names.unwrap();
        assert_eq!(names, vec!["tool_a", "tool_b"]);
    }

    #[test]
    fn tool_config_none_mode_roundtrip() {
        let tc = GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode: GeminiFunctionCallingMode::None,
                allowed_function_names: None,
            },
        };
        let json = serde_json::to_string(&tc).expect("ser");
        assert!(json.contains("NONE"));
    }
}

mod gemini_tool_tests {
    use super::*;

    #[test]
    fn google_search_tool_roundtrip() {
        let tool = GeminiTool {
            function_declarations: None,
            google_search: Some(GoogleSearch::default()),
            url_context: None,
            code_execution: None,
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("googleSearch"));
        assert!(!json.contains("functionDeclarations"));
    }

    #[test]
    fn url_context_tool_roundtrip() {
        let tool = GeminiTool {
            function_declarations: None,
            google_search: None,
            url_context: Some(UrlContext::default()),
            code_execution: None,
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("urlContext"));
    }

    #[test]
    fn code_execution_tool_roundtrip() {
        let tool = GeminiTool {
            function_declarations: None,
            google_search: None,
            url_context: None,
            code_execution: Some(CodeExecution::default()),
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("codeExecution"));
    }

    #[test]
    fn function_declarations_tool_roundtrip() {
        let tool = GeminiTool {
            function_declarations: Some(vec![GeminiFunctionDeclaration {
                name: "get_weather".to_owned(),
                description: Some("Get current weather".to_owned()),
                parameters: json!({"type": "object"}),
            }]),
            google_search: None,
            url_context: None,
            code_execution: None,
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("functionDeclarations"));
        assert!(json.contains("get_weather"));
    }

    #[test]
    fn safety_setting_roundtrip() {
        let setting = GeminiSafetySetting {
            category: "HARM_CATEGORY_HATE_SPEECH".to_owned(),
            threshold: "BLOCK_NONE".to_owned(),
        };
        let json = serde_json::to_string(&setting).expect("ser");
        let back: GeminiSafetySetting = serde_json::from_str(&json).expect("de");
        assert_eq!(back.category, "HARM_CATEGORY_HATE_SPEECH");
        assert_eq!(back.threshold, "BLOCK_NONE");
    }
}

mod gemini_response_tests {
    use super::*;

    #[test]
    fn usage_metadata_roundtrip() {
        let usage = GeminiUsageMetadata {
            prompt: 50,
            candidates: Some(120),
            total: 170,
        };
        let json = serde_json::to_string(&usage).expect("ser");
        assert!(json.contains("promptTokenCount"));
        assert!(json.contains("totalTokenCount"));
        let back: GeminiUsageMetadata = serde_json::from_str(&json).expect("de");
        assert_eq!(back.prompt, 50);
        assert_eq!(back.candidates, Some(120));
        assert_eq!(back.total, 170);
    }

    #[test]
    fn usage_metadata_no_candidates() {
        let json = r#"{"promptTokenCount":10,"totalTokenCount":10}"#;
        let usage: GeminiUsageMetadata = serde_json::from_str(json).expect("de");
        assert_eq!(usage.prompt, 10);
        assert!(usage.candidates.is_none());
        assert_eq!(usage.total, 10);
    }

    #[test]
    fn grounding_metadata_roundtrip() {
        let gm = GeminiGroundingMetadata {
            grounding_chunks: vec![GeminiGroundingChunk {
                web: GeminiWebSource {
                    uri: "https://example.com".to_owned(),
                    title: "Example".to_owned(),
                },
            }],
            grounding_supports: vec![GeminiGroundingSupport {
                segment: GeminiTextSegment {
                    start_index: 0,
                    end_index: 10,
                    text: "some text".to_owned(),
                },
                grounding_chunk_indices: vec![0],
                confidence_scores: vec![0.95],
            }],
            web_search_queries: vec!["weather today".to_owned()],
        };
        let json = serde_json::to_string(&gm).expect("ser");
        let back: GeminiGroundingMetadata = serde_json::from_str(&json).expect("de");
        assert_eq!(back.grounding_chunks.len(), 1);
        assert_eq!(back.grounding_chunks[0].web.uri, "https://example.com");
        assert_eq!(back.grounding_supports[0].confidence_scores[0], 0.95);
        assert_eq!(back.web_search_queries[0], "weather today");
    }

    #[test]
    fn url_context_metadata_roundtrip() {
        let meta = GeminiUrlContextMetadata {
            url_metadata: vec![GeminiUrlMetadata {
                retrieved_url: "https://example.com/page".to_owned(),
                url_retrieval_status: "SUCCESS".to_owned(),
            }],
        };
        let json = serde_json::to_string(&meta).expect("ser");
        let back: GeminiUrlContextMetadata = serde_json::from_str(&json).expect("de");
        assert_eq!(back.url_metadata.len(), 1);
        assert_eq!(back.url_metadata[0].retrieved_url, "https://example.com/page");
        assert_eq!(back.url_metadata[0].url_retrieval_status, "SUCCESS");
    }

    #[test]
    fn full_response_with_grounding_roundtrip() {
        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: Some(GeminiContent {
                    role: "model".to_owned(),
                    parts: vec![GeminiPart::Text {
                        text: "The weather is sunny.".to_owned(),
                    }],
                }),
                finish_reason: Some("STOP".to_owned()),
                index: Some(0),
                safety_ratings: Some(vec![GeminiSafetyRating {
                    category: "HARM_CATEGORY_HARASSMENT".to_owned(),
                    probability: "NEGLIGIBLE".to_owned(),
                }]),
                grounding_metadata: Some(GeminiGroundingMetadata {
                    grounding_chunks: vec![],
                    grounding_supports: vec![],
                    web_search_queries: vec!["weather".to_owned()],
                }),
                url_context_metadata: None,
            }],
            usage_metadata: Some(GeminiUsageMetadata {
                prompt: 100,
                candidates: Some(50),
                total: 150,
            }),
        };
        let json = serde_json::to_string(&response).expect("ser");
        let back: GeminiResponse = serde_json::from_str(&json).expect("de");
        assert_eq!(back.candidates.len(), 1);
        assert_eq!(back.usage_metadata.unwrap().total, 150);
        let candidate = &back.candidates[0];
        assert_eq!(candidate.finish_reason.as_deref(), Some("STOP"));
        let gm = candidate.grounding_metadata.as_ref().unwrap();
        assert_eq!(gm.web_search_queries[0], "weather");
    }

    #[test]
    fn response_no_candidates() {
        let response = GeminiResponse {
            candidates: vec![],
            usage_metadata: None,
        };
        let json = serde_json::to_string(&response).expect("ser");
        let back: GeminiResponse = serde_json::from_str(&json).expect("de");
        assert!(back.candidates.is_empty());
        assert!(back.usage_metadata.is_none());
    }

    #[test]
    fn gemini_request_roundtrip() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_owned(),
                parts: vec![GeminiPart::Text {
                    text: "Hello".to_owned(),
                }],
            }],
            generation_config: Some(GeminiGenerationConfig {
                temperature: Some(0.5),
                top_p: None,
                top_k: None,
                max_output_tokens: Some(256),
                stop_sequences: None,
                response_mime_type: None,
                response_schema: None,
                response_modalities: None,
                image_config: None,
                thinking_config: None,
            }),
            safety_settings: None,
            tools: None,
            tool_config: None,
        };
        let json = serde_json::to_string(&request).expect("ser");
        assert!(json.contains("contents"));
        assert!(json.contains("generationConfig"));
        let back: GeminiRequest = serde_json::from_str(&json).expect("de");
        assert_eq!(back.contents.len(), 1);
        assert_eq!(back.contents[0].role, "user");
    }
}
