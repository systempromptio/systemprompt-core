use systemprompt_wire::gemini::{
    GeminiContent, GeminiEmpty, GeminiGenerationConfig, GeminiImageConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiResponse, GeminiTool,
};

fn text(value: &str) -> GeminiPart {
    GeminiPart::Text {
        text: value.to_owned(),
        thought: None,
        thought_signature: None,
    }
}

mod gemini_part_variants {
    use super::*;

    #[test]
    fn text_part_serializes_without_thought_fields() {
        let json = serde_json::to_string(&text("Test text")).expect("ser");
        assert_eq!(json, r#"{"text":"Test text"}"#);
    }

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
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn content_roundtrip_keeps_parts_in_order() {
        let content = GeminiContent {
            role: "user".to_owned(),
            parts: vec![text("First part"), text("Second part")],
        };
        let json = serde_json::to_string(&content).expect("ser");
        let back: GeminiContent = serde_json::from_str(&json).expect("de");
        assert_eq!(back.role, "user");
        assert_eq!(back.parts.len(), 2);
        assert!(matches!(&back.parts[1], GeminiPart::Text { text, .. } if text == "Second part"));
    }
}

mod gemini_generation_config_tests {
    use super::*;

    #[test]
    fn image_modalities_config_serializes_only_set_fields() {
        let cfg = GeminiGenerationConfig {
            response_modalities: Some(vec!["IMAGE".to_owned()]),
            image_config: Some(GeminiImageConfig {
                aspect_ratio: "1:1".to_owned(),
                image_size: Some("1K".to_owned()),
            }),
            ..GeminiGenerationConfig::default()
        };
        let json: serde_json::Value = serde_json::to_value(&cfg).expect("ser");
        assert_eq!(
            json,
            serde_json::json!({
                "responseModalities": ["IMAGE"],
                "imageConfig": {"aspectRatio": "1:1", "imageSize": "1K"}
            })
        );
    }

    #[test]
    fn image_config_skips_absent_size() {
        let ic = GeminiImageConfig {
            aspect_ratio: "16:9".to_owned(),
            image_size: None,
        };
        let json = serde_json::to_string(&ic).expect("ser");
        assert_eq!(json, r#"{"aspectRatio":"16:9"}"#);
    }
}

mod gemini_tool_tests {
    use super::*;

    #[test]
    fn google_search_tool_is_an_empty_object_marker() {
        let tool = GeminiTool::GoogleSearch {
            google_search: GeminiEmpty {},
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert_eq!(json, r#"{"googleSearch":{}}"#);
    }
}

mod gemini_request_response_tests {
    use super::*;

    #[test]
    fn image_request_omits_unset_sections() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_owned(),
                parts: vec![text("a red square")],
            }],
            system_instruction: None,
            generation_config: Some(GeminiGenerationConfig {
                response_modalities: Some(vec!["IMAGE".to_owned()]),
                ..GeminiGenerationConfig::default()
            }),
            tools: None,
            tool_config: None,
        };
        let json: serde_json::Value = serde_json::to_value(&request).expect("ser");
        assert_eq!(json["contents"][0]["role"], "user");
        assert_eq!(json["generationConfig"]["responseModalities"][0], "IMAGE");
        assert!(json.get("tools").is_none());
        assert!(json.get("systemInstruction").is_none());
    }

    #[test]
    fn response_candidate_carries_inline_image() {
        let json = r#"{"candidates":[{"content":{"role":"model","parts":[
            {"inlineData":{"mimeType":"image/png","data":"AAAA"}}]}}]}"#;
        let back: GeminiResponse = serde_json::from_str(json).expect("de");
        let content = back.candidates[0].content.as_ref().expect("content");
        assert!(matches!(content.parts[0], GeminiPart::InlineData { .. }));
    }

    #[test]
    fn response_tolerates_unknown_text_fields() {
        let json = r#"{
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": "ok"}]},
                "finishReason": "STOP",
                "safetyRatings": []
            }],
            "usageMetadata": {"promptTokenCount": 1, "totalTokenCount": 1}
        }"#;
        let back: GeminiResponse = serde_json::from_str(json).expect("de");
        assert_eq!(back.candidates.len(), 1);
    }
}
