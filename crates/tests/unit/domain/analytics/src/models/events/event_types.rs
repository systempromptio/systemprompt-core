//! Tests for analytics event type enum.

use systemprompt_analytics::AnalyticsEventType;

mod analytics_event_type_tests {
    use super::*;

    #[test]
    fn page_view_as_str() {
        assert_eq!(AnalyticsEventType::PageView.as_str(), "page_view");
    }

    #[test]
    fn page_exit_as_str() {
        assert_eq!(AnalyticsEventType::PageExit.as_str(), "page_exit");
    }

    #[test]
    fn link_click_as_str() {
        assert_eq!(AnalyticsEventType::LinkClick.as_str(), "link_click");
    }

    #[test]
    fn scroll_as_str() {
        assert_eq!(AnalyticsEventType::Scroll.as_str(), "scroll");
    }

    #[test]
    fn engagement_as_str() {
        assert_eq!(AnalyticsEventType::Engagement.as_str(), "engagement");
    }

    #[test]
    fn conversion_as_str() {
        assert_eq!(AnalyticsEventType::Conversion.as_str(), "conversion");
    }

    #[test]
    fn custom_as_str() {
        let custom = AnalyticsEventType::Custom("my_custom_event".to_string());
        assert_eq!(custom.as_str(), "my_custom_event");
    }

    #[test]
    fn page_view_category() {
        assert_eq!(AnalyticsEventType::PageView.category(), "navigation");
    }

    #[test]
    fn page_exit_category() {
        assert_eq!(AnalyticsEventType::PageExit.category(), "navigation");
    }

    #[test]
    fn link_click_category() {
        assert_eq!(AnalyticsEventType::LinkClick.category(), "interaction");
    }

    #[test]
    fn scroll_category() {
        assert_eq!(AnalyticsEventType::Scroll.category(), "engagement");
    }

    #[test]
    fn engagement_category() {
        assert_eq!(AnalyticsEventType::Engagement.category(), "engagement");
    }

    #[test]
    fn conversion_category() {
        assert_eq!(AnalyticsEventType::Conversion.category(), "conversion");
    }

    #[test]
    fn custom_category() {
        let custom = AnalyticsEventType::Custom("test".to_string());
        assert_eq!(custom.category(), "custom");
    }

    #[test]
    fn display_matches_as_str() {
        let types = [
            AnalyticsEventType::PageView,
            AnalyticsEventType::PageExit,
            AnalyticsEventType::LinkClick,
            AnalyticsEventType::Scroll,
            AnalyticsEventType::Engagement,
            AnalyticsEventType::Conversion,
            AnalyticsEventType::Custom("test".to_string()),
        ];

        for event_type in types {
            assert_eq!(format!("{}", event_type), event_type.as_str());
        }
    }

    #[test]
    fn event_type_serializes() {
        let event_type = AnalyticsEventType::PageView;
        let json = serde_json::to_string(&event_type).unwrap();
        assert!(json.contains("page_view"));
    }

    #[test]
    fn event_type_is_debug() {
        let debug_str = format!("{:?}", AnalyticsEventType::Scroll);
        assert!(debug_str.contains("Scroll"));
    }
}
