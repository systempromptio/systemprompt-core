use systemprompt_ai::services::tooled::{
    FallbackGenerator, FallbackReason, ResponseSynthesizer, SynthesisPromptBuilder,
};
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::MessageRole;
use systemprompt_identifiers::AiToolCallId;
use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;

fn create_tool_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{}", name)),
        name: name.to_string(),
        arguments: json!({}),
    }
}

fn create_text_content(text: &str) -> Content {
    Annotated {
        raw: RawContent::Text(RawTextContent {
            text: text.to_string(),
            meta: None,
        }),
        annotations: None,
    }
}

fn create_success_result(text: &str) -> CallToolResult {
    CallToolResult {
        content: vec![create_text_content(text)],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    }
}

fn create_error_result(text: &str) -> CallToolResult {
    CallToolResult {
        content: vec![create_text_content(text)],
        structured_content: None,
        is_error: Some(true),
        meta: None,
    }
}

mod fallback_reason_tests {
    use super::*;

    #[test]
    fn empty_content_is_debug() {
        let reason = FallbackReason::EmptyContent;
        let debug_str = format!("{:?}", reason);
        assert!(debug_str.contains("EmptyContent"));
    }

    #[test]
    fn synthesis_failed_is_debug() {
        let reason = FallbackReason::SynthesisFailed("test error".to_string());
        let debug_str = format!("{:?}", reason);
        assert!(debug_str.contains("SynthesisFailed"));
        assert!(debug_str.contains("test error"));
    }
}

mod fallback_generator_tests {
    use super::*;

    #[test]
    fn new_creates_generator() {
        let generator = FallbackGenerator::new();
        let _ = format!("{:?}", generator);
    }

    #[test]
    fn default_creates_generator() {
        let generator = FallbackGenerator::default();
        let _ = format!("{:?}", generator);
    }

    #[test]
    fn is_copy() {
        let generator = FallbackGenerator::new();
        let copied = generator;
        let _ = format!("{:?}", copied);
    }

    #[test]
    fn is_clone() {
        let generator = FallbackGenerator::new();
        let cloned = generator.clone();
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn generate_with_empty_content_reason() {
        let tool_calls = vec![create_tool_call("search")];
        let tool_results = vec![create_success_result("Found 5 results")];

        let output = FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Tool execution completed"));
        assert!(!output.contains("Synthesis error"));
    }

    #[test]
    fn generate_with_synthesis_failed_reason() {
        let tool_calls = vec![create_tool_call("search")];
        let tool_results = vec![create_success_result("Found 5 results")];

        let output = FallbackGenerator::generate(
            &tool_calls,
            &tool_results,
            FallbackReason::SynthesisFailed("Provider timeout".to_string()),
        );

        assert!(output.contains("Tool execution completed"));
        assert!(output.contains("Synthesis error"));
        assert!(output.contains("Provider timeout"));
    }

    #[test]
    fn generate_with_multiple_tools() {
        let tool_calls = vec![
            create_tool_call("search"),
            create_tool_call("fetch"),
        ];
        let tool_results = vec![
            create_success_result("Search results"),
            create_success_result("Page content"),
        ];

        let output = FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Tool execution completed"));
    }

    #[test]
    fn generate_with_error_results() {
        let tool_calls = vec![create_tool_call("fetch")];
        let tool_results = vec![create_error_result("Connection refused")];

        let output = FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Tool execution completed"));
    }

    #[test]
    fn generate_with_empty_tools() {
        let tool_calls: Vec<ToolCall> = vec![];
        let tool_results: Vec<CallToolResult> = vec![];

        let output = FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Tool execution completed"));
    }
}

mod synthesis_prompt_builder_tests {
    use super::*;

    #[test]
    fn build_guidance_message_creates_user_message() {
        let tool_calls = vec![create_tool_call("search")];
        let tool_results = vec![create_success_result("Found 5 results")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert_eq!(message.role, MessageRole::User);
    }

    #[test]
    fn build_guidance_message_includes_instructions() {
        let tool_calls = vec![create_tool_call("search")];
        let tool_results = vec![create_success_result("Found 5 results")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.content.contains("tool"));
        assert!(message.content.contains("executed"));
        assert!(message.content.contains("response"));
    }

    #[test]
    fn build_guidance_message_has_empty_parts() {
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("result")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.parts.is_empty());
    }

    #[test]
    fn build_guidance_message_with_multiple_tools() {
        let tool_calls = vec![
            create_tool_call("search"),
            create_tool_call("analyze"),
        ];
        let tool_results = vec![
            create_success_result("Search results"),
            create_success_result("Analysis complete"),
        ];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(!message.content.is_empty());
    }

    #[test]
    fn build_guidance_message_with_error_result() {
        let tool_calls = vec![create_tool_call("fetch")];
        let tool_results = vec![create_error_result("Network error")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(!message.content.is_empty());
    }
}

mod response_synthesizer_tests {
    use super::*;

    #[test]
    fn new_creates_synthesizer() {
        let synthesizer = ResponseSynthesizer::new();
        let _ = format!("{:?}", synthesizer);
    }

    #[test]
    fn default_creates_synthesizer() {
        let synthesizer = ResponseSynthesizer::default();
        let _ = format!("{:?}", synthesizer);
    }

    #[test]
    fn is_copy() {
        let synthesizer = ResponseSynthesizer::new();
        let copied = synthesizer;
        let _ = format!("{:?}", copied);
    }

    #[test]
    fn is_clone() {
        let synthesizer = ResponseSynthesizer::new();
        let cloned = synthesizer.clone();
        let _ = format!("{:?}", cloned);
    }
}
