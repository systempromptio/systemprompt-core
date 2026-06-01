use systemprompt_ai::models::providers::anthropic::{
    AnthropicContent, AnthropicSearchContentBlock, AnthropicSearchRequest, AnthropicSearchResponse,
    AnthropicSearchUsage, AnthropicServerTool, AnthropicWebSearchResultItem,
};

mod server_tool_tests {
    use super::*;

    #[test]
    fn web_search_tool_serializes_correctly() {
        let tool = AnthropicServerTool::WebSearch {
            name: "web_search".to_owned(),
            max_uses: Some(5),
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("web_search_20250305"));
        assert!(json.contains("web_search"));
        assert!(json.contains("max_uses"));
    }

    #[test]
    fn web_search_tool_no_max_uses_skips_field() {
        let tool = AnthropicServerTool::WebSearch {
            name: "web_search".to_owned(),
            max_uses: None,
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(!json.contains("max_uses"));
    }

    #[test]
    fn web_search_tool_roundtrip_via_json() {
        let tool = AnthropicServerTool::WebSearch {
            name: "web_search".to_owned(),
            max_uses: Some(10),
        };
        let json = serde_json::to_string(&tool).expect("ser");
        let back: AnthropicServerTool = serde_json::from_str(&json).expect("de");
        match back {
            AnthropicServerTool::WebSearch { name, max_uses } => {
                assert_eq!(name, "web_search");
                assert_eq!(max_uses, Some(10));
            },
        }
    }
}

mod search_request_tests {
    use super::*;

    #[test]
    fn minimal_request_serializes() {
        let req = AnthropicSearchRequest {
            model: "claude-sonnet-4-6-20250610".to_owned(),
            messages: vec![],
            max_tokens: 512,
            temperature: None,
            top_p: None,
            top_k: None,
            system: None,
            tools: vec![],
        };
        let json = serde_json::to_string(&req).expect("ser");
        assert!(json.contains("claude-sonnet-4-6-20250610"));
        assert!(!json.contains("temperature"));
        assert!(!json.contains("system"));
    }

    #[test]
    fn request_with_sampling_params() {
        let req = AnthropicSearchRequest {
            model: "claude-opus-4-6-20250610".to_owned(),
            messages: vec![],
            max_tokens: 2048,
            temperature: Some(0.3),
            top_p: Some(0.9),
            top_k: Some(50),
            system: Some("You are a helpful assistant.".to_owned()),
            tools: vec![],
        };
        let json = serde_json::to_string(&req).expect("ser");
        assert!(json.contains("temperature"));
        assert!(json.contains("top_p"));
        assert!(json.contains("top_k"));
        assert!(json.contains("system"));
    }

    #[test]
    fn request_with_user_message() {
        use systemprompt_ai::models::providers::anthropic::AnthropicMessage;
        let req = AnthropicSearchRequest {
            model: "claude-sonnet-4-6-20250610".to_owned(),
            messages: vec![AnthropicMessage {
                role: "user".to_owned(),
                content: AnthropicContent::Text("What's the weather?".to_owned()),
            }],
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            system: None,
            tools: vec![AnthropicServerTool::WebSearch {
                name: "web_search".to_owned(),
                max_uses: Some(5),
            }],
        };
        let json = serde_json::to_string(&req).expect("ser");
        assert!(json.contains("weather"));
        assert!(json.contains("web_search"));
    }
}

mod anthropic_search_content_block_tests {
    use super::*;

    #[test]
    fn text_block_deserializes() {
        let json = r#"{"type": "text", "text": "The weather is sunny."}"#;
        let block: AnthropicSearchContentBlock = serde_json::from_str(json).expect("de");
        match block {
            AnthropicSearchContentBlock::Text { text, citations } => {
                assert_eq!(text, "The weather is sunny.");
                assert!(citations.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn text_block_with_citations() {
        let json = r#"{
            "type": "text",
            "text": "The sky is blue.",
            "citations": [
                {
                    "type": "web_search_result",
                    "url": "https://example.com",
                    "title": "Example Page",
                    "cited_text": "sky is blue"
                }
            ]
        }"#;
        let block: AnthropicSearchContentBlock = serde_json::from_str(json).expect("de");
        match block {
            AnthropicSearchContentBlock::Text { citations, .. } => {
                let cites = citations.expect("has citations");
                assert_eq!(cites.len(), 1);
                assert_eq!(cites[0].url, "https://example.com");
                assert_eq!(cites[0].title, "Example Page");
                assert_eq!(cites[0].cited_text.as_deref(), Some("sky is blue"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn citation_no_cited_text() {
        let json = r#"{
            "type": "text",
            "text": "fact",
            "citations": [{"type": "web", "url": "https://example.com", "title": "Page"}]
        }"#;
        let block: AnthropicSearchContentBlock = serde_json::from_str(json).expect("de");
        match block {
            AnthropicSearchContentBlock::Text { citations, .. } => {
                let cites = citations.expect("has citations");
                assert!(cites[0].cited_text.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn server_tool_use_block_deserializes() {
        let json = r#"{
            "type": "server_tool_use",
            "id": "tu_123",
            "name": "web_search",
            "input": {"query": "weather today"}
        }"#;
        let block: AnthropicSearchContentBlock = serde_json::from_str(json).expect("de");
        match block {
            AnthropicSearchContentBlock::ServerToolUse { id, name, input } => {
                assert_eq!(id, "tu_123");
                assert_eq!(name, "web_search");
                assert_eq!(input["query"], "weather today");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn web_search_tool_result_block_deserializes() {
        let json = r#"{
            "type": "web_search_tool_result",
            "tool_use_id": "tu_123",
            "content": [
                {
                    "type": "web_search_result",
                    "url": "https://weather.com",
                    "title": "Current Weather",
                    "page_age": "2026-01-01"
                }
            ]
        }"#;
        let block: AnthropicSearchContentBlock = serde_json::from_str(json).expect("de");
        match block {
            AnthropicSearchContentBlock::WebSearchToolResult {
                tool_use_id,
                content,
            } => {
                assert_eq!(tool_use_id, "tu_123");
                assert_eq!(content.len(), 1);
                match &content[0] {
                    AnthropicWebSearchResultItem::WebSearchResult {
                        url,
                        title,
                        page_age,
                    } => {
                        assert_eq!(url, "https://weather.com");
                        assert_eq!(title, "Current Weather");
                        assert_eq!(page_age.as_deref(), Some("2026-01-01"));
                    },
                    _ => panic!("wrong content variant"),
                }
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn web_search_result_error_item() {
        let json = r#"{"type": "web_search_tool_result_error", "error_code": "TOO_MANY_REQUESTS"}"#;
        let item: AnthropicWebSearchResultItem = serde_json::from_str(json).expect("de");
        match item {
            AnthropicWebSearchResultItem::Error { error_code } => {
                assert_eq!(error_code, "TOO_MANY_REQUESTS");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn web_search_result_no_page_age() {
        let json = r#"{
            "type": "web_search_result",
            "url": "https://example.com",
            "title": "Page"
        }"#;
        let item: AnthropicWebSearchResultItem = serde_json::from_str(json).expect("de");
        match item {
            AnthropicWebSearchResultItem::WebSearchResult { page_age, .. } => {
                assert!(page_age.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }
}

mod search_usage_tests {
    use super::*;

    #[test]
    fn usage_deserialization() {
        let json = r#"{"input_tokens": 150, "output_tokens": 300}"#;
        let usage: AnthropicSearchUsage = serde_json::from_str(json).expect("de");
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, 300);
        assert!(usage.server_tool_use.is_none());
    }

    #[test]
    fn usage_with_server_tool_use() {
        let json = r#"{
            "input_tokens": 100,
            "output_tokens": 200,
            "server_tool_use": {"web_search_requests": 3}
        }"#;
        let usage: AnthropicSearchUsage = serde_json::from_str(json).expect("de");
        assert_eq!(usage.input_tokens, 100);
        let stu = usage.server_tool_use.expect("has server_tool_use");
        assert_eq!(stu.web_search_requests, 3);
    }

    #[test]
    fn usage_with_zero_server_tool_use() {
        let json = r#"{
            "input_tokens": 50,
            "output_tokens": 80,
            "server_tool_use": {"web_search_requests": 0}
        }"#;
        let usage: AnthropicSearchUsage = serde_json::from_str(json).expect("de");
        let stu = usage.server_tool_use.expect("has server_tool_use");
        assert_eq!(stu.web_search_requests, 0);
    }
}

mod search_response_tests {
    use super::*;

    #[test]
    fn full_search_response_deserializes() {
        let json = r#"{
            "id": "msg_search_01",
            "content": [
                {"type": "text", "text": "The answer is 42."}
            ],
            "model": "claude-sonnet-4-6-20250610",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 50, "output_tokens": 10}
        }"#;
        let response: AnthropicSearchResponse = serde_json::from_str(json).expect("de");
        assert_eq!(response.id, "msg_search_01");
        assert_eq!(response.content.len(), 1);
        assert_eq!(response.model, "claude-sonnet-4-6-20250610");
        assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(response.usage.input_tokens, 50);
        assert_eq!(response.usage.output_tokens, 10);
    }

    #[test]
    fn search_response_no_stop_reason() {
        let json = r#"{
            "id": "msg_002",
            "content": [],
            "model": "claude-haiku-4-5-20251101",
            "usage": {"input_tokens": 5, "output_tokens": 0}
        }"#;
        let response: AnthropicSearchResponse = serde_json::from_str(json).expect("de");
        assert!(response.stop_reason.is_none());
        assert!(response.content.is_empty());
    }

    #[test]
    fn search_response_with_multiple_content_blocks() {
        let json = r#"{
            "id": "msg_003",
            "content": [
                {"type": "server_tool_use", "id": "tu_1", "name": "web_search", "input": {"query": "rust lang"}},
                {"type": "web_search_tool_result", "tool_use_id": "tu_1", "content": []},
                {"type": "text", "text": "Rust is a systems programming language."}
            ],
            "model": "claude-sonnet-4-6-20250610",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 100, "output_tokens": 30}
        }"#;
        let response: AnthropicSearchResponse = serde_json::from_str(json).expect("de");
        assert_eq!(response.content.len(), 3);
        assert!(matches!(
            &response.content[0],
            AnthropicSearchContentBlock::ServerToolUse { .. }
        ));
        assert!(matches!(
            &response.content[2],
            AnthropicSearchContentBlock::Text { .. }
        ));
    }
}
