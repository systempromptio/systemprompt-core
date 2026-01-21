//! Tests for image generation models.

use systemprompt_ai::models::image_generation::{
    AspectRatio, GeneratedImageRecord, ImageGenerationRequest, ImageGenerationResponse,
    ImageResolution, NewImageGenerationResponse, ReferenceImage,
};

mod image_resolution_tests {
    use super::*;

    #[test]
    fn default_resolution_is_one_k() {
        let resolution = ImageResolution::default();
        assert_eq!(resolution, ImageResolution::OneK);
    }

    #[test]
    fn one_k_as_str() {
        assert_eq!(ImageResolution::OneK.as_str(), "1K");
    }

    #[test]
    fn two_k_as_str() {
        assert_eq!(ImageResolution::TwoK.as_str(), "2K");
    }

    #[test]
    fn four_k_as_str() {
        assert_eq!(ImageResolution::FourK.as_str(), "4K");
    }

    #[test]
    fn resolution_equality() {
        assert_eq!(ImageResolution::OneK, ImageResolution::OneK);
        assert_eq!(ImageResolution::TwoK, ImageResolution::TwoK);
        assert_eq!(ImageResolution::FourK, ImageResolution::FourK);
        assert_ne!(ImageResolution::OneK, ImageResolution::TwoK);
    }

    #[test]
    fn resolution_is_copy() {
        let res = ImageResolution::TwoK;
        let copied = res;
        assert_eq!(res, copied);
    }

    #[test]
    fn resolution_serialization() {
        let res = ImageResolution::TwoK;
        let json = serde_json::to_string(&res).unwrap();
        assert_eq!(json, "\"2K\"");
    }

    #[test]
    fn resolution_deserialization() {
        let res: ImageResolution = serde_json::from_str("\"4K\"").unwrap();
        assert_eq!(res, ImageResolution::FourK);
    }
}

mod aspect_ratio_tests {
    use super::*;

    #[test]
    fn default_aspect_ratio_is_square() {
        let ratio = AspectRatio::default();
        assert_eq!(ratio, AspectRatio::Square);
    }

    #[test]
    fn square_as_str() {
        assert_eq!(AspectRatio::Square.as_str(), "1:1");
    }

    #[test]
    fn landscape_16_9_as_str() {
        assert_eq!(AspectRatio::Landscape169.as_str(), "16:9");
    }

    #[test]
    fn portrait_9_16_as_str() {
        assert_eq!(AspectRatio::Portrait916.as_str(), "9:16");
    }

    #[test]
    fn landscape_4_3_as_str() {
        assert_eq!(AspectRatio::Landscape43.as_str(), "4:3");
    }

    #[test]
    fn portrait_3_4_as_str() {
        assert_eq!(AspectRatio::Portrait34.as_str(), "3:4");
    }

    #[test]
    fn ultra_wide_as_str() {
        assert_eq!(AspectRatio::UltraWide.as_str(), "21:9");
    }

    #[test]
    fn aspect_ratio_equality() {
        assert_eq!(AspectRatio::Square, AspectRatio::Square);
        assert_eq!(AspectRatio::Landscape169, AspectRatio::Landscape169);
        assert_ne!(AspectRatio::Square, AspectRatio::Landscape169);
    }

    #[test]
    fn aspect_ratio_is_copy() {
        let ratio = AspectRatio::Portrait916;
        let copied = ratio;
        assert_eq!(ratio, copied);
    }

    #[test]
    fn aspect_ratio_serialization() {
        let ratio = AspectRatio::Landscape169;
        let json = serde_json::to_string(&ratio).unwrap();
        assert_eq!(json, "\"16:9\"");
    }

    #[test]
    fn aspect_ratio_deserialization() {
        let ratio: AspectRatio = serde_json::from_str("\"9:16\"").unwrap();
        assert_eq!(ratio, AspectRatio::Portrait916);
    }
}

mod image_generation_request_tests {
    use super::*;

    #[test]
    fn create_minimal_request() {
        let request = ImageGenerationRequest {
            prompt: "A beautiful sunset".to_string(),
            model: None,
            resolution: ImageResolution::default(),
            aspect_ratio: AspectRatio::default(),
            reference_images: Vec::new(),
            enable_search_grounding: false,
            user_id: None,
            session_id: None,
            trace_id: None,
            mcp_execution_id: None,
        };

        assert_eq!(request.prompt, "A beautiful sunset");
        assert!(request.model.is_none());
        assert_eq!(request.resolution, ImageResolution::OneK);
        assert_eq!(request.aspect_ratio, AspectRatio::Square);
        assert!(request.reference_images.is_empty());
        assert!(!request.enable_search_grounding);
    }

    #[test]
    fn create_full_request() {
        let reference = ReferenceImage {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
            description: Some("Reference image".to_string()),
        };

        let request = ImageGenerationRequest {
            prompt: "Generate like the reference".to_string(),
            model: Some("gemini-pro".to_string()),
            resolution: ImageResolution::FourK,
            aspect_ratio: AspectRatio::Landscape169,
            reference_images: vec![reference],
            enable_search_grounding: true,
            user_id: Some("user-123".to_string()),
            session_id: Some("session-456".to_string()),
            trace_id: Some("trace-789".to_string()),
            mcp_execution_id: None,
        };

        assert_eq!(request.prompt, "Generate like the reference");
        assert_eq!(request.model, Some("gemini-pro".to_string()));
        assert_eq!(request.resolution, ImageResolution::FourK);
        assert_eq!(request.aspect_ratio, AspectRatio::Landscape169);
        assert_eq!(request.reference_images.len(), 1);
        assert!(request.enable_search_grounding);
        assert_eq!(request.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn request_serialization_roundtrip() {
        let request = ImageGenerationRequest {
            prompt: "Test prompt".to_string(),
            model: Some("model-name".to_string()),
            resolution: ImageResolution::TwoK,
            aspect_ratio: AspectRatio::Portrait34,
            reference_images: Vec::new(),
            enable_search_grounding: false,
            user_id: None,
            session_id: None,
            trace_id: None,
            mcp_execution_id: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ImageGenerationRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.prompt, deserialized.prompt);
        assert_eq!(request.model, deserialized.model);
        assert_eq!(request.resolution, deserialized.resolution);
        assert_eq!(request.aspect_ratio, deserialized.aspect_ratio);
    }
}

mod reference_image_tests {
    use super::*;

    #[test]
    fn create_reference_image() {
        let ref_img = ReferenceImage {
            data: "base64encodeddata".to_string(),
            mime_type: "image/jpeg".to_string(),
            description: Some("A sample reference".to_string()),
        };

        assert_eq!(ref_img.data, "base64encodeddata");
        assert_eq!(ref_img.mime_type, "image/jpeg");
        assert_eq!(ref_img.description, Some("A sample reference".to_string()));
    }

    #[test]
    fn reference_image_without_description() {
        let ref_img = ReferenceImage {
            data: "data".to_string(),
            mime_type: "image/png".to_string(),
            description: None,
        };

        assert!(ref_img.description.is_none());
    }

    #[test]
    fn reference_image_serialization() {
        let ref_img = ReferenceImage {
            data: "abc123".to_string(),
            mime_type: "image/webp".to_string(),
            description: None,
        };

        let json = serde_json::to_string(&ref_img).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("image/webp"));
    }
}

mod image_generation_response_tests {
    use super::*;

    #[test]
    fn new_creates_response_with_generated_ids() {
        let params = NewImageGenerationResponse {
            provider: "gemini".to_string(),
            model: "gemini-pro-vision".to_string(),
            image_data: "base64imagedata".to_string(),
            mime_type: "image/png".to_string(),
            resolution: ImageResolution::TwoK,
            aspect_ratio: AspectRatio::Landscape169,
            generation_time_ms: 1500,
        };

        let response = ImageGenerationResponse::new(params);

        assert!(!response.id.is_empty());
        assert!(!response.request_id.is_empty());
        assert_eq!(response.provider, "gemini");
        assert_eq!(response.model, "gemini-pro-vision");
        assert_eq!(response.image_data, "base64imagedata");
        assert_eq!(response.mime_type, "image/png");
        assert_eq!(response.resolution, ImageResolution::TwoK);
        assert_eq!(response.aspect_ratio, AspectRatio::Landscape169);
        assert_eq!(response.generation_time_ms, 1500);
        assert!(response.file_path.is_none());
        assert!(response.public_url.is_none());
        assert!(response.file_size_bytes.is_none());
        assert!(response.cost_estimate.is_none());
    }

    #[test]
    fn new_sets_created_at() {
        let params = NewImageGenerationResponse {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            image_data: "data".to_string(),
            mime_type: "image/png".to_string(),
            resolution: ImageResolution::default(),
            aspect_ratio: AspectRatio::default(),
            generation_time_ms: 100,
        };

        let before = chrono::Utc::now();
        let response = ImageGenerationResponse::new(params);
        let after = chrono::Utc::now();

        assert!(response.created_at >= before);
        assert!(response.created_at <= after);
    }

    #[test]
    fn response_serialization() {
        let params = NewImageGenerationResponse {
            provider: "provider".to_string(),
            model: "model".to_string(),
            image_data: "imagedata".to_string(),
            mime_type: "image/jpeg".to_string(),
            resolution: ImageResolution::OneK,
            aspect_ratio: AspectRatio::Square,
            generation_time_ms: 500,
        };

        let response = ImageGenerationResponse::new(params);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("provider"));
        assert!(json.contains("model"));
        assert!(json.contains("imagedata"));
    }
}

mod generated_image_record_tests {
    use super::*;

    #[test]
    fn create_generated_image_record() {
        let record = GeneratedImageRecord {
            uuid: "uuid-123".to_string(),
            request_id: "req-456".to_string(),
            prompt: "Test prompt".to_string(),
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
            file_path: "/images/test.png".to_string(),
            public_url: "https://example.com/images/test.png".to_string(),
            file_size_bytes: Some(1024),
            mime_type: "image/png".to_string(),
            resolution: Some("1K".to_string()),
            aspect_ratio: Some("1:1".to_string()),
            generation_time_ms: Some(500),
            cost_estimate: Some(0.05),
            user_id: Some("user-123".to_string()),
            session_id: Some("session-456".to_string()),
            trace_id: None,
            created_at: chrono::Utc::now(),
            expires_at: None,
            deleted_at: None,
        };

        assert_eq!(record.uuid, "uuid-123");
        assert_eq!(record.request_id, "req-456");
        assert_eq!(record.prompt, "Test prompt");
        assert_eq!(record.file_size_bytes, Some(1024));
    }

    #[test]
    fn record_with_expiration() {
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::days(30);

        let record = GeneratedImageRecord {
            uuid: "uuid".to_string(),
            request_id: "req".to_string(),
            prompt: "prompt".to_string(),
            model: "model".to_string(),
            provider: "provider".to_string(),
            file_path: "/path".to_string(),
            public_url: "https://url".to_string(),
            file_size_bytes: None,
            mime_type: "image/png".to_string(),
            resolution: None,
            aspect_ratio: None,
            generation_time_ms: None,
            cost_estimate: None,
            user_id: None,
            session_id: None,
            trace_id: None,
            created_at: now,
            expires_at: Some(expires),
            deleted_at: None,
        };

        assert!(record.expires_at.is_some());
        assert!(record.deleted_at.is_none());
    }

    #[test]
    fn record_serialization_roundtrip() {
        let record = GeneratedImageRecord {
            uuid: "uuid-test".to_string(),
            request_id: "req-test".to_string(),
            prompt: "test prompt".to_string(),
            model: "model".to_string(),
            provider: "provider".to_string(),
            file_path: "/path/to/image.png".to_string(),
            public_url: "https://example.com/image.png".to_string(),
            file_size_bytes: Some(2048),
            mime_type: "image/png".to_string(),
            resolution: Some("2K".to_string()),
            aspect_ratio: Some("16:9".to_string()),
            generation_time_ms: Some(1000),
            cost_estimate: Some(0.10),
            user_id: Some("user".to_string()),
            session_id: None,
            trace_id: None,
            created_at: chrono::Utc::now(),
            expires_at: None,
            deleted_at: None,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: GeneratedImageRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(record.uuid, deserialized.uuid);
        assert_eq!(record.prompt, deserialized.prompt);
        assert_eq!(record.file_size_bytes, deserialized.file_size_bytes);
    }
}
