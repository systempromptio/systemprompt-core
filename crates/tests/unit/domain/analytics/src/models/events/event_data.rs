//! Tests for analytics event data types.

use systemprompt_analytics::{
    ConversionEventData, EngagementEventData, LinkClickEventData, ScrollEventData,
};

mod engagement_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = EngagementEventData::default();

        assert!(data.max_scroll_depth.is_none());
        assert!(data.time_on_page_ms.is_none());
        assert!(data.time_to_first_interaction_ms.is_none());
        assert!(data.time_to_first_scroll_ms.is_none());
        assert!(data.click_count.is_none());
        assert!(data.scroll_velocity_avg.is_none());
        assert!(data.scroll_direction_changes.is_none());
        assert!(data.mouse_move_distance_px.is_none());
        assert!(data.keyboard_events.is_none());
        assert!(data.copy_events.is_none());
        assert!(data.focus_time_ms.is_none());
        assert!(data.blur_count.is_none());
        assert!(data.tab_switches.is_none());
        assert!(data.visible_time_ms.is_none());
        assert!(data.hidden_time_ms.is_none());
        assert!(data.is_rage_click.is_none());
        assert!(data.is_dead_click.is_none());
        assert!(data.reading_pattern.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = EngagementEventData {
            max_scroll_depth: Some(75),
            time_on_page_ms: Some(30000),
            click_count: Some(5),
            is_rage_click: Some(true),
            reading_pattern: Some("scanning".to_string()),
            ..Default::default()
        };

        assert_eq!(data.max_scroll_depth, Some(75));
        assert_eq!(data.time_on_page_ms, Some(30000));
        assert_eq!(data.click_count, Some(5));
        assert_eq!(data.is_rage_click, Some(true));
        assert_eq!(data.reading_pattern, Some("scanning".to_string()));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = EngagementEventData {
            max_scroll_depth: Some(50),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("max_scroll_depth"));
        assert!(!json.contains("time_on_page_ms"));
        assert!(!json.contains("is_rage_click"));
    }

    #[test]
    fn data_is_debug() {
        let data = EngagementEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("EngagementEventData"));
    }
}

mod link_click_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = LinkClickEventData::default();

        assert!(data.target_url.is_none());
        assert!(data.link_text.is_none());
        assert!(data.link_position.is_none());
        assert!(data.is_external.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = LinkClickEventData {
            target_url: Some("https://example.com".to_string()),
            link_text: Some("Click here".to_string()),
            link_position: Some("header".to_string()),
            is_external: Some(true),
        };

        assert_eq!(data.target_url, Some("https://example.com".to_string()));
        assert_eq!(data.link_text, Some("Click here".to_string()));
        assert_eq!(data.link_position, Some("header".to_string()));
        assert_eq!(data.is_external, Some(true));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = LinkClickEventData {
            target_url: Some("https://test.com".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("target_url"));
        assert!(!json.contains("link_text"));
    }

    #[test]
    fn data_is_debug() {
        let data = LinkClickEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("LinkClickEventData"));
    }
}

mod scroll_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = ScrollEventData::default();

        assert!(data.depth.is_none());
        assert!(data.milestone.is_none());
        assert!(data.direction.is_none());
        assert!(data.velocity.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = ScrollEventData {
            depth: Some(50),
            milestone: Some(25),
            direction: Some("down".to_string()),
            velocity: Some(1.5),
        };

        assert_eq!(data.depth, Some(50));
        assert_eq!(data.milestone, Some(25));
        assert_eq!(data.direction, Some("down".to_string()));
        assert!((data.velocity.unwrap() - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = ScrollEventData {
            depth: Some(75),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("depth"));
        assert!(!json.contains("milestone"));
    }

    #[test]
    fn data_is_debug() {
        let data = ScrollEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("ScrollEventData"));
    }
}

mod conversion_event_data_tests {
    use super::*;

    #[test]
    fn default_creates_all_none() {
        let data = ConversionEventData::default();

        assert!(data.goal_name.is_none());
        assert!(data.goal_value.is_none());
        assert!(data.funnel_step.is_none());
    }

    #[test]
    fn data_with_values() {
        let data = ConversionEventData {
            goal_name: Some("signup".to_string()),
            goal_value: Some(99.99),
            funnel_step: Some(3),
        };

        assert_eq!(data.goal_name, Some("signup".to_string()));
        assert!((data.goal_value.unwrap() - 99.99).abs() < f64::EPSILON);
        assert_eq!(data.funnel_step, Some(3));
    }

    #[test]
    fn data_serializes_skipping_none() {
        let data = ConversionEventData {
            goal_name: Some("purchase".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("goal_name"));
        assert!(!json.contains("goal_value"));
    }

    #[test]
    fn data_is_debug() {
        let data = ConversionEventData::default();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("ConversionEventData"));
    }
}
