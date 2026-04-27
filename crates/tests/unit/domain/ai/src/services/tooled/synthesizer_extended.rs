use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::MessageRole;
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::tooled::{
    FallbackGenerator, FallbackReason, ResponseSynthesizer, SynthesisPromptBuilder,
};
use systemprompt_identifiers::AiToolCallId;

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
    CallToolResult::success(vec![create_text_content(text)])
}

fn create_error_result(text: &str) -> CallToolResult {
    CallToolResult::error(vec![create_text_content(text)])
}

fn create_empty_result() -> CallToolResult {
    CallToolResult::success(vec![])
}

mod fallback_generator_extended_tests {
    use super::*;

    #[test]
    fn new_returns_fallback_generator() {
        let generator = FallbackGenerator::new();
        let debug = format!("{:?}", generator);
        assert!(debug.contains("FallbackGenerator"));
    }

    #[test]
    fn default_returns_fallback_generator() {
        let generator = FallbackGenerator::default();
        let debug = format!("{:?}", generator);
        assert!(debug.contains("FallbackGenerator"));
    }

    #[test]
    fn generate_mixed_success_and_error_only_shows_success() {
        let tool_calls = vec![create_tool_call("good_tool"), create_tool_call("bad_tool")];
        let tool_results = vec![
            create_success_result("Good data here"),
            create_error_result("Something went wrong"),
        ];

        let output =
            FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Good data here"));
        assert!(!output.contains("Something went wrong"));
    }

    #[test]
    fn generate_with_empty_content_results_shows_completed() {
        let tool_calls = vec![create_tool_call("empty_tool")];
        let tool_results = vec![create_empty_result()];

        let output =
            FallbackGenerator::generate(&tool_calls, &tool_results, FallbackReason::EmptyContent);

        assert!(output.contains("Tool execution completed"));
    }

    #[test]
    fn synthesis_failed_includes_error_message() {
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("data")];

        let output = FallbackGenerator::generate(
            &tool_calls,
            &tool_results,
            FallbackReason::SynthesisFailed("Rate limit exceeded".to_string()),
        );

        assert!(output.contains("Rate limit exceeded"));
        assert!(output.contains("Synthesis error"));
    }

    #[test]
    fn synthesis_failed_with_empty_error_string() {
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("data")];

        let output = FallbackGenerator::generate(
            &tool_calls,
            &tool_results,
            FallbackReason::SynthesisFailed(String::new()),
        );

        assert!(output.contains("Synthesis error"));
    }
}

mod synthesis_prompt_builder_extended_tests {
    use super::*;

    #[test]
    fn guidance_message_includes_tool_display_data() {
        let tool_calls = vec![create_tool_call("weather")];
        let tool_results = vec![create_success_result("72F and sunny")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.content.contains("weather"));
        assert!(message.content.contains("72F and sunny") || message.content.contains("SUCCESS"));
    }

    #[test]
    fn guidance_message_with_error_result_includes_failed_status() {
        let tool_calls = vec![create_tool_call("api_call")];
        let tool_results = vec![create_error_result("503 Service Unavailable")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.content.contains("FAILED"));
    }

    #[test]
    fn guidance_message_with_empty_tools() {
        let message = SynthesisPromptBuilder::build_guidance_message(&[], &[]);

        assert_eq!(message.role, MessageRole::User);
        assert!(message.content.contains("tool"));
    }

    #[test]
    fn guidance_message_mentions_synthesis_instructions() {
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("result")];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.content.contains("natural language"));
        assert!(message.content.contains("concise"));
    }

    #[test]
    fn guidance_message_with_multiple_mixed_results() {
        let tool_calls = vec![
            create_tool_call("search"),
            create_tool_call("compute"),
            create_tool_call("fetch"),
        ];
        let tool_results = vec![
            create_success_result("Found 5 documents"),
            create_error_result("Division by zero"),
            create_success_result("Page loaded OK"),
        ];

        let message = SynthesisPromptBuilder::build_guidance_message(&tool_calls, &tool_results);

        assert!(message.content.contains("search"));
        assert!(message.content.contains("compute"));
        assert!(message.content.contains("fetch"));
    }
}

mod response_synthesizer_construction_tests {
    use super::*;

    #[test]
    fn new_creates_synthesizer() {
        let synthesizer = ResponseSynthesizer::new();
        let debug = format!("{:?}", synthesizer);
        assert!(debug.contains("ResponseSynthesizer"));
    }

    #[test]
    fn default_creates_synthesizer() {
        let synthesizer = ResponseSynthesizer::default();
        let debug = format!("{:?}", synthesizer);
        assert!(debug.contains("ResponseSynthesizer"));
    }

    #[test]
    fn is_copy() {
        let synthesizer = ResponseSynthesizer::new();
        let copied = synthesizer;
        let _still_valid = synthesizer;
        let _ = format!("{:?}", copied);
    }
}

mod fallback_reason_tests {
    use super::*;

    #[test]
    fn empty_content_debug_format() {
        let reason = FallbackReason::EmptyContent;
        let debug = format!("{:?}", reason);
        assert_eq!(debug, "EmptyContent");
    }

    #[test]
    fn synthesis_failed_debug_contains_message() {
        let reason = FallbackReason::SynthesisFailed("timeout after 30s".to_string());
        let debug = format!("{:?}", reason);
        assert!(debug.contains("timeout after 30s"));
    }

    #[test]
    fn synthesis_failed_with_long_error() {
        let long_error = "x".repeat(1000);
        let reason = FallbackReason::SynthesisFailed(long_error.clone());
        let debug = format!("{:?}", reason);
        assert!(debug.contains(&long_error));
    }
}
