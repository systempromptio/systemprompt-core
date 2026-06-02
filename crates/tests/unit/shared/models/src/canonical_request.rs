use serde_json::json;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalStopReason, ImageDetail,
    ImageSource, ReasoningEffort, Role,
};

fn empty_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".to_owned(),
        system: None,
        messages: Vec::new(),
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

fn msg(role: Role, content: Vec<CanonicalContent>) -> CanonicalMessage {
    CanonicalMessage { role, content }
}

mod role_as_str {
    use super::*;

    #[test]
    fn maps_every_role() {
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
        assert_eq!(Role::Tool.as_str(), "tool");
    }
}

mod image_detail_as_str {
    use super::*;

    #[test]
    fn maps_every_variant() {
        assert_eq!(ImageDetail::Auto.as_str(), "auto");
        assert_eq!(ImageDetail::Low.as_str(), "low");
        assert_eq!(ImageDetail::High.as_str(), "high");
    }
}

mod reasoning_effort_as_str {
    use super::*;

    #[test]
    fn maps_every_variant() {
        assert_eq!(ReasoningEffort::Low.as_str(), "low");
        assert_eq!(ReasoningEffort::Medium.as_str(), "medium");
        assert_eq!(ReasoningEffort::High.as_str(), "high");
    }
}

mod stop_reason_mapping {
    use super::*;

    #[test]
    fn anthropic_str_for_each_variant() {
        assert_eq!(CanonicalStopReason::MaxTokens.anthropic_str(), "max_tokens");
        assert_eq!(
            CanonicalStopReason::StopSequence.anthropic_str(),
            "stop_sequence"
        );
        assert_eq!(CanonicalStopReason::ToolUse.anthropic_str(), "tool_use");
        assert_eq!(CanonicalStopReason::EndTurn.anthropic_str(), "end_turn");
        assert_eq!(CanonicalStopReason::Other.anthropic_str(), "end_turn");
    }

    #[test]
    fn openai_str_for_each_variant() {
        assert_eq!(CanonicalStopReason::MaxTokens.openai_str(), "length");
        assert_eq!(CanonicalStopReason::ToolUse.openai_str(), "tool_calls");
        assert_eq!(CanonicalStopReason::EndTurn.openai_str(), "stop");
        assert_eq!(CanonicalStopReason::StopSequence.openai_str(), "stop");
        assert_eq!(CanonicalStopReason::Other.openai_str(), "stop");
    }

    #[test]
    fn from_anthropic_known_and_unknown() {
        assert_eq!(
            CanonicalStopReason::from_anthropic("end_turn"),
            CanonicalStopReason::EndTurn
        );
        assert_eq!(
            CanonicalStopReason::from_anthropic("max_tokens"),
            CanonicalStopReason::MaxTokens
        );
        assert_eq!(
            CanonicalStopReason::from_anthropic("stop_sequence"),
            CanonicalStopReason::StopSequence
        );
        assert_eq!(
            CanonicalStopReason::from_anthropic("tool_use"),
            CanonicalStopReason::ToolUse
        );
        assert_eq!(
            CanonicalStopReason::from_anthropic("anything-else"),
            CanonicalStopReason::Other
        );
    }

    #[test]
    fn from_openai_known_and_unknown() {
        assert_eq!(
            CanonicalStopReason::from_openai("stop"),
            CanonicalStopReason::EndTurn
        );
        assert_eq!(
            CanonicalStopReason::from_openai("length"),
            CanonicalStopReason::MaxTokens
        );
        assert_eq!(
            CanonicalStopReason::from_openai("tool_calls"),
            CanonicalStopReason::ToolUse
        );
        assert_eq!(
            CanonicalStopReason::from_openai("function_call"),
            CanonicalStopReason::ToolUse
        );
        assert_eq!(
            CanonicalStopReason::from_openai("content_filter"),
            CanonicalStopReason::Other
        );
    }
}

mod flatten_text {
    use super::*;

    #[test]
    fn empty_request_flattens_to_empty() {
        assert_eq!(empty_request().flatten_text(), "");
    }

    #[test]
    fn system_prompt_leads_the_flattened_text() {
        let mut req = empty_request();
        req.system = Some("you are helpful".to_owned());
        req.messages = vec![msg(Role::User, vec![CanonicalContent::Text("hi".to_owned())])];
        assert_eq!(req.flatten_text(), "you are helpful\nhi");
    }

    #[test]
    fn images_are_skipped_in_flatten() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::User,
            vec![
                CanonicalContent::Text("before".to_owned()),
                CanonicalContent::Image(ImageSource::Url {
                    url: "https://x".to_owned(),
                    detail: Some(ImageDetail::Auto),
                }),
                CanonicalContent::Text("after".to_owned()),
            ],
        )];
        assert_eq!(req.flatten_text(), "before\nafter");
    }

    #[test]
    fn thinking_text_is_included() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::Assistant,
            vec![CanonicalContent::Thinking {
                text: "pondering".to_owned(),
                signature: None,
            }],
        )];
        assert_eq!(req.flatten_text(), "pondering");
    }

    #[test]
    fn tool_use_renders_name_and_input() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::Assistant,
            vec![CanonicalContent::ToolUse {
                id: "c1".to_owned(),
                name: "search".to_owned(),
                input: json!({"q": "rust"}),
                signature: None,
            }],
        )];
        let text = req.flatten_text();
        assert!(text.contains("[tool_use:search"));
        assert!(text.contains("rust"));
    }

    #[test]
    fn tool_result_flattens_inner_content_recursively() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::Tool,
            vec![CanonicalContent::ToolResult {
                tool_use_id: "c1".to_owned(),
                content: vec![CanonicalContent::Text("result body".to_owned())],
                is_error: false,
                structured_content: None,
                meta: None,
            }],
        )];
        assert_eq!(req.flatten_text(), "result body");
    }

    #[test]
    fn empty_text_fragments_do_not_add_separators() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::User,
            vec![
                CanonicalContent::Text(String::new()),
                CanonicalContent::Text("only".to_owned()),
            ],
        )];
        assert_eq!(req.flatten_text(), "only");
    }
}

mod flatten_message_text {
    use super::*;

    #[test]
    fn returns_none_when_no_message_of_role() {
        let mut req = empty_request();
        req.messages = vec![msg(Role::User, vec![CanonicalContent::Text("hi".to_owned())])];
        assert!(req.flatten_message_text(Role::Assistant).is_none());
    }

    #[test]
    fn collects_only_matching_role() {
        let mut req = empty_request();
        req.messages = vec![
            msg(Role::User, vec![CanonicalContent::Text("u1".to_owned())]),
            msg(Role::Assistant, vec![CanonicalContent::Text("a1".to_owned())]),
            msg(Role::User, vec![CanonicalContent::Text("u2".to_owned())]),
        ];
        assert_eq!(req.flatten_message_text(Role::User), Some("u1\nu2".to_owned()));
        assert_eq!(req.flatten_message_text(Role::Assistant), Some("a1".to_owned()));
    }

    #[test]
    fn returns_none_when_matching_message_has_only_images() {
        let mut req = empty_request();
        req.messages = vec![msg(
            Role::User,
            vec![CanonicalContent::Image(ImageSource::Base64 {
                media_type: "image/png".to_owned(),
                data: "QQ==".to_owned(),
                detail: None,
            })],
        )];
        assert!(req.flatten_message_text(Role::User).is_none());
    }
}

mod derived_gateway_conversation_id {
    use super::*;

    #[test]
    fn none_when_no_messages() {
        assert!(empty_request().derived_gateway_conversation_id().is_none());
    }

    #[test]
    fn deterministic_for_same_leading_message() {
        let mut req = empty_request();
        req.system = Some("sys".to_owned());
        req.messages = vec![msg(Role::User, vec![CanonicalContent::Text("hello".to_owned())])];
        let a = req.derived_gateway_conversation_id().expect("id");
        let b = req.derived_gateway_conversation_id().expect("id");
        assert_eq!(a, b);
    }

    #[test]
    fn differs_when_leading_content_differs() {
        let mut req_a = empty_request();
        req_a.messages = vec![msg(Role::User, vec![CanonicalContent::Text("alpha".to_owned())])];
        let mut req_b = empty_request();
        req_b.messages = vec![msg(Role::User, vec![CanonicalContent::Text("beta".to_owned())])];
        assert_ne!(
            req_a.derived_gateway_conversation_id(),
            req_b.derived_gateway_conversation_id()
        );
    }
}
