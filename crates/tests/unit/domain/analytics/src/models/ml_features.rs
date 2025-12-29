//! Tests for ML features model types.

use systemprompt_core_analytics::FeatureExtractionConfig;

mod feature_extraction_config_tests {
    use super::*;

    #[test]
    fn default_enables_all_features() {
        let config = FeatureExtractionConfig::default();

        assert!(config.include_session_features);
        assert!(config.include_navigation_features);
        assert!(config.include_behavioral_features);
        assert!(config.include_timing_features);
        assert!(config.normalize_features);
    }

    #[test]
    fn config_is_copy() {
        let config = FeatureExtractionConfig::default();
        let copied = config;

        assert_eq!(config.include_session_features, copied.include_session_features);
        assert_eq!(config.normalize_features, copied.normalize_features);
    }

    #[test]
    fn config_is_clone() {
        let config = FeatureExtractionConfig::default();
        let cloned = config.clone();

        assert_eq!(config.include_session_features, cloned.include_session_features);
        assert_eq!(config.include_navigation_features, cloned.include_navigation_features);
    }

    #[test]
    fn config_is_debug() {
        let config = FeatureExtractionConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("FeatureExtractionConfig"));
        assert!(debug_str.contains("include_session_features"));
    }

    #[test]
    fn custom_config_can_disable_features() {
        let config = FeatureExtractionConfig {
            include_session_features: false,
            include_navigation_features: true,
            include_behavioral_features: false,
            include_timing_features: true,
            normalize_features: false,
        };

        assert!(!config.include_session_features);
        assert!(config.include_navigation_features);
        assert!(!config.include_behavioral_features);
        assert!(config.include_timing_features);
        assert!(!config.normalize_features);
    }
}
