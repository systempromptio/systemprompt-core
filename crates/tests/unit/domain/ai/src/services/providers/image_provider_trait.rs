use systemprompt_ai::services::providers::ImageProviderCapabilities;
use systemprompt_ai::{AspectRatio, ImageResolution};

mod image_provider_capabilities_tests {
    use super::*;

    #[test]
    fn default_capabilities_structure() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![ImageResolution::OneK, ImageResolution::TwoK],
            supported_aspect_ratios: vec![AspectRatio::Square, AspectRatio::Landscape169],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 1000,
            cost_per_image_cents: 5.0,
        };

        assert_eq!(caps.supported_resolutions.len(), 2);
        assert_eq!(caps.supported_aspect_ratios.len(), 2);
        assert!(!caps.supports_batch);
        assert!(!caps.supports_image_editing);
        assert!(!caps.supports_search_grounding);
        assert_eq!(caps.max_prompt_length, 1000);
        assert!((caps.cost_per_image_cents - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn capabilities_with_batch_support() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: true,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        assert!(caps.supports_batch);
    }

    #[test]
    fn capabilities_with_editing_support() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: true,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        assert!(caps.supports_image_editing);
    }

    #[test]
    fn capabilities_with_search_grounding() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: true,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        assert!(caps.supports_search_grounding);
    }

    #[test]
    fn capabilities_all_resolutions() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![
                ImageResolution::OneK,
                ImageResolution::TwoK,
                ImageResolution::FourK,
            ],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        assert_eq!(caps.supported_resolutions.len(), 3);
        assert!(caps.supported_resolutions.contains(&ImageResolution::OneK));
        assert!(caps.supported_resolutions.contains(&ImageResolution::TwoK));
        assert!(caps.supported_resolutions.contains(&ImageResolution::FourK));
    }

    #[test]
    fn capabilities_all_aspect_ratios() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![
                AspectRatio::Square,
                AspectRatio::Landscape169,
                AspectRatio::Portrait916,
                AspectRatio::Landscape43,
                AspectRatio::Portrait34,
                AspectRatio::UltraWide,
            ],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        assert_eq!(caps.supported_aspect_ratios.len(), 6);
        assert!(caps.supported_aspect_ratios.contains(&AspectRatio::Square));
        assert!(caps.supported_aspect_ratios.contains(&AspectRatio::UltraWide));
    }

    #[test]
    fn capabilities_is_clone() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![ImageResolution::OneK],
            supported_aspect_ratios: vec![AspectRatio::Square],
            supports_batch: true,
            supports_image_editing: true,
            supports_search_grounding: true,
            max_prompt_length: 5000,
            cost_per_image_cents: 10.5,
        };

        let cloned = caps.clone();

        assert_eq!(cloned.supported_resolutions.len(), 1);
        assert_eq!(cloned.supported_aspect_ratios.len(), 1);
        assert!(cloned.supports_batch);
        assert!(cloned.supports_image_editing);
        assert!(cloned.supports_search_grounding);
        assert_eq!(cloned.max_prompt_length, 5000);
        assert!((cloned.cost_per_image_cents - 10.5).abs() < f32::EPSILON);
    }

    #[test]
    fn capabilities_is_debug() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 0.0,
        };

        let debug_str = format!("{:?}", caps);
        assert!(debug_str.contains("ImageProviderCapabilities"));
    }

    #[test]
    fn capabilities_with_high_cost() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 0,
            cost_per_image_cents: 100.0,
        };

        assert!((caps.cost_per_image_cents - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn capabilities_with_large_prompt_length() {
        let caps = ImageProviderCapabilities {
            supported_resolutions: vec![],
            supported_aspect_ratios: vec![],
            supports_batch: false,
            supports_image_editing: false,
            supports_search_grounding: false,
            max_prompt_length: 100_000,
            cost_per_image_cents: 0.0,
        };

        assert_eq!(caps.max_prompt_length, 100_000);
    }
}

mod gemini_image_provider_tests {
    use systemprompt_ai::GeminiImageProvider;
    use systemprompt_ai::services::providers::ImageProvider;

    #[test]
    fn new_creates_provider() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "gemini-image");
    }

    #[test]
    fn with_endpoint_creates_provider() {
        let provider = GeminiImageProvider::with_endpoint(
            "test-key".to_string(),
            "https://custom.endpoint.com".to_string(),
        );
        assert_eq!(provider.name(), "gemini-image");
    }

    #[test]
    fn default_model_is_set() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        assert!(!provider.default_model().is_empty());
    }

    #[test]
    fn with_default_model_changes_model() {
        let provider = GeminiImageProvider::new("test-key".to_string())
            .with_default_model("custom-model".to_string());
        assert_eq!(provider.default_model(), "custom-model");
    }

    #[test]
    fn capabilities_returns_valid_caps() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        let caps = provider.capabilities();

        assert!(!caps.supported_resolutions.is_empty());
        assert!(!caps.supported_aspect_ratios.is_empty());
        assert!(caps.max_prompt_length > 0);
    }

    #[test]
    fn supported_models_returns_list() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        let models = provider.supported_models();

        assert!(!models.is_empty());
    }

    #[test]
    fn supports_model_returns_true_for_known_model() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        let models = provider.supported_models();

        if let Some(model) = models.first() {
            assert!(provider.supports_model(model));
        }
    }

    #[test]
    fn supports_model_returns_false_for_unknown_model() {
        let provider = GeminiImageProvider::new("test-key".to_string());
        assert!(!provider.supports_model("unknown-model-xyz"));
    }

    #[test]
    fn supports_resolution_checks_capabilities() {
        use systemprompt_ai::ImageResolution;

        let provider = GeminiImageProvider::new("test-key".to_string());
        let caps = provider.capabilities();

        for resolution in &caps.supported_resolutions {
            assert!(provider.supports_resolution(resolution));
        }
    }

    #[test]
    fn supports_aspect_ratio_checks_capabilities() {
        use systemprompt_ai::AspectRatio;

        let provider = GeminiImageProvider::new("test-key".to_string());
        let caps = provider.capabilities();

        for ratio in &caps.supported_aspect_ratios {
            assert!(provider.supports_aspect_ratio(ratio));
        }
    }
}
