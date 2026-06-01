use systemprompt_ai::models::providers::gemini::{
    GeminiCandidate, GeminiContent, GeminiGenerationConfig, GeminiImageConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiResponse, GeminiTool, GoogleSearch,
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
            GeminiPart::Text { .. } => panic!("wrong variant"),
        }
    }
}

mod gemini_generation_config_tests {
    use super::*;

    #[test]
    fn image_modalities_config_roundtrip() {
        let cfg = GeminiGenerationConfig {
            temperature: None,
            top_p: None,
            top_k: None,
            max_output_tokens: None,
            stop_sequences: None,
            response_mime_type: None,
            response_schema: None,
            response_modalities: Some(vec!["IMAGE".to_owned()]),
            image_config: Some(GeminiImageConfig {
                aspect_ratio: "1:1".to_owned(),
                image_size: Some("1K".to_owned()),
            }),
        };
        let json = serde_json::to_string(&cfg).expect("ser");
        assert!(json.contains("responseModalities"));
        assert!(json.contains("imageConfig"));
        assert!(!json.contains("temperature"));
        let back: GeminiGenerationConfig = serde_json::from_str(&json).expect("de");
        assert_eq!(
            back.response_modalities.as_deref(),
            Some(&["IMAGE".to_owned()][..])
        );
        assert_eq!(back.image_config.unwrap().aspect_ratio, "1:1");
    }

    #[test]
    fn image_config_skips_absent_size() {
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

mod gemini_tool_tests {
    use super::*;

    #[test]
    fn google_search_tool_roundtrip() {
        let tool = GeminiTool {
            google_search: Some(GoogleSearch::default()),
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert!(json.contains("googleSearch"));
    }

    #[test]
    fn tool_without_search_is_empty_object() {
        let tool = GeminiTool {
            google_search: None,
        };
        let json = serde_json::to_string(&tool).expect("ser");
        assert_eq!(json, "{}");
    }
}

mod gemini_request_response_tests {
    use super::*;

    #[test]
    fn image_request_roundtrip() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_owned(),
                parts: vec![GeminiPart::Text {
                    text: "a red square".to_owned(),
                }],
            }],
            generation_config: Some(GeminiGenerationConfig {
                temperature: None,
                top_p: None,
                top_k: None,
                max_output_tokens: None,
                stop_sequences: None,
                response_mime_type: None,
                response_schema: None,
                response_modalities: Some(vec!["IMAGE".to_owned()]),
                image_config: None,
            }),
            tools: None,
        };
        let json = serde_json::to_string(&request).expect("ser");
        assert!(json.contains("contents"));
        assert!(json.contains("generationConfig"));
        let back: GeminiRequest = serde_json::from_str(&json).expect("de");
        assert_eq!(back.contents[0].role, "user");
    }

    #[test]
    fn response_candidate_carries_inline_image() {
        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: Some(GeminiContent {
                    role: "model".to_owned(),
                    parts: vec![GeminiPart::InlineData {
                        inline_data: GeminiInlineData {
                            mime_type: "image/png".to_owned(),
                            data: "AAAA".to_owned(),
                        },
                    }],
                }),
            }],
        };
        let json = serde_json::to_string(&response).expect("ser");
        let back: GeminiResponse = serde_json::from_str(&json).expect("de");
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
