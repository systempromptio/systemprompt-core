//! Tests for analytics event input types.

use systemprompt_analytics::{
    AnalyticsEventType, CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput,
};

mod create_analytics_event_input_tests {
    use super::*;

    fn create_input(event_type: AnalyticsEventType, page_url: &str) -> CreateAnalyticsEventInput {
        CreateAnalyticsEventInput {
            event_type,
            page_url: page_url.to_string(),
            content_id: None,
            slug: None,
            referrer: None,
            data: None,
        }
    }

    #[test]
    fn input_stores_event_type() {
        let input = create_input(AnalyticsEventType::PageView, "/home");
        assert_eq!(input.event_type, AnalyticsEventType::PageView);
    }

    #[test]
    fn input_stores_page_url() {
        let input = create_input(AnalyticsEventType::Scroll, "/about");
        assert_eq!(input.page_url, "/about");
    }

    #[test]
    fn input_with_all_fields() {
        let data = serde_json::json!({"key": "value"});
        let input = CreateAnalyticsEventInput {
            event_type: AnalyticsEventType::Conversion,
            page_url: "/checkout".to_string(),
            content_id: Some(systemprompt_identifiers::ContentId::new("cnt_123".to_string())),
            slug: Some("checkout-page".to_string()),
            referrer: Some("https://google.com".to_string()),
            data: Some(data.clone()),
        };

        assert_eq!(input.event_type, AnalyticsEventType::Conversion);
        assert_eq!(input.page_url, "/checkout");
        input.content_id.expect("content_id should be set");
        assert_eq!(input.slug, Some("checkout-page".to_string()));
        assert_eq!(input.referrer, Some("https://google.com".to_string()));
        input.data.expect("data should be set");
    }

    #[test]
    fn input_is_debug() {
        let input = create_input(AnalyticsEventType::Engagement, "/test");
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("CreateAnalyticsEventInput"));
    }

    #[test]
    fn input_deserializes_with_custom_data() {
        let json = r#"{
            "event_type": "conversion",
            "page_url": "/thank-you",
            "data": {"amount": 199.99, "currency": "USD"}
        }"#;

        let input: CreateAnalyticsEventInput = serde_json::from_str(json).unwrap();

        let data = input.data.expect("data should be present for conversion event");
        assert_eq!(data["amount"], 199.99);
        assert_eq!(data["currency"], "USD");
    }
}

mod create_analytics_event_batch_input_tests {
    use super::*;

    #[test]
    fn batch_stores_events() {
        let events = vec![
            CreateAnalyticsEventInput {
                event_type: AnalyticsEventType::PageView,
                page_url: "/page1".to_string(),
                content_id: None,
                slug: None,
                referrer: None,
                data: None,
            },
            CreateAnalyticsEventInput {
                event_type: AnalyticsEventType::Scroll,
                page_url: "/page1".to_string(),
                content_id: None,
                slug: None,
                referrer: None,
                data: None,
            },
        ];

        let batch = CreateAnalyticsEventBatchInput { events };

        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].event_type, AnalyticsEventType::PageView);
        assert_eq!(batch.events[1].event_type, AnalyticsEventType::Scroll);
    }

    #[test]
    fn batch_is_debug() {
        let batch = CreateAnalyticsEventBatchInput { events: vec![] };
        let debug_str = format!("{:?}", batch);
        assert!(debug_str.contains("CreateAnalyticsEventBatchInput"));
    }
}
