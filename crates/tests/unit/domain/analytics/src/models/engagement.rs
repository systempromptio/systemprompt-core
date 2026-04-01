//! Tests for engagement model types.

use systemprompt_analytics::{CreateEngagementEventInput, EngagementOptionalMetrics};

mod engagement_optional_metrics_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let metrics = EngagementOptionalMetrics::default();

        assert!(metrics.time_to_first_interaction_ms.is_none());
        assert!(metrics.time_to_first_scroll_ms.is_none());
        assert!(metrics.scroll_velocity_avg.is_none());
        assert!(metrics.scroll_direction_changes.is_none());
        assert!(metrics.mouse_move_distance_px.is_none());
        assert!(metrics.keyboard_events.is_none());
        assert!(metrics.copy_events.is_none());
        assert!(metrics.focus_time_ms.is_none());
        assert!(metrics.blur_count.is_none());
        assert!(metrics.visible_time_ms.is_none());
        assert!(metrics.hidden_time_ms.is_none());
        assert!(metrics.is_rage_click.is_none());
        assert!(metrics.is_dead_click.is_none());
        assert!(metrics.reading_pattern.is_none());
    }

    #[test]
    fn metrics_is_debug() {
        let metrics = EngagementOptionalMetrics::default();
        let debug_str = format!("{:?}", metrics);

        assert!(debug_str.contains("EngagementOptionalMetrics"));
    }
}

mod create_engagement_event_input_tests {
    use super::*;

    fn create_input(page_url: &str, time_ms: i32, scroll: i32, clicks: i32) -> CreateEngagementEventInput {
        CreateEngagementEventInput {
            page_url: page_url.to_string(),
            time_on_page_ms: time_ms,
            max_scroll_depth: scroll,
            click_count: clicks,
            ..Default::default()
        }
    }

    #[test]
    fn input_stores_required_fields() {
        let input = create_input("/about", 5000, 75, 3);

        assert_eq!(input.page_url, "/about");
        assert_eq!(input.time_on_page_ms, 5000);
        assert_eq!(input.max_scroll_depth, 75);
        assert_eq!(input.click_count, 3);
    }

    #[test]
    fn input_with_optional_metrics() {
        let input = CreateEngagementEventInput {
            page_url: "/product".to_string(),
            time_on_page_ms: 10000,
            max_scroll_depth: 100,
            click_count: 5,
            optional_metrics: EngagementOptionalMetrics {
                time_to_first_interaction_ms: Some(200),
                is_dead_click: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(input.optional_metrics.time_to_first_interaction_ms, Some(200));
        assert_eq!(input.optional_metrics.is_dead_click, Some(true));
        assert!(input.optional_metrics.is_rage_click.is_none());
    }

    #[test]
    fn input_is_debug() {
        let input = create_input("/test", 500, 25, 1);
        let debug_str = format!("{:?}", input);

        assert!(debug_str.contains("CreateEngagementEventInput"));
        assert!(debug_str.contains("/test"));
    }

    #[test]
    fn input_handles_zero_values() {
        let input = create_input("/", 0, 0, 0);

        assert_eq!(input.time_on_page_ms, 0);
        assert_eq!(input.max_scroll_depth, 0);
        assert_eq!(input.click_count, 0);
    }

    #[test]
    fn input_handles_large_values() {
        let input = create_input("/long-session", i32::MAX, 100, 1000);

        assert_eq!(input.time_on_page_ms, i32::MAX);
        assert_eq!(input.max_scroll_depth, 100);
        assert_eq!(input.click_count, 1000);
    }
}
