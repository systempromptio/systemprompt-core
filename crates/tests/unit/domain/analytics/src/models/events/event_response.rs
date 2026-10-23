//! Tests for analytics event response types.

use systemprompt_analytics::{AnalyticsEventBatchResponse, AnalyticsEventCreated};
use systemprompt_identifiers::AnalyticsEventId;

mod analytics_event_created_tests {
    use super::*;

    #[test]
    fn created_stores_id_and_type() {
        let created = AnalyticsEventCreated {
            id: AnalyticsEventId::new("evt_123"),
            event_type: "page_view".to_string(),
        };

        assert_eq!(created.id.as_str(), "evt_123");
        assert_eq!(created.event_type, "page_view");
    }

    #[test]
    fn created_serializes() {
        let created = AnalyticsEventCreated {
            id: AnalyticsEventId::new("evt_456"),
            event_type: "scroll".to_string(),
        };

        let json = serde_json::to_string(&created).unwrap();
        assert!(json.contains("evt_456"));
        assert!(json.contains("scroll"));
    }
}

mod analytics_event_batch_response_tests {
    use super::*;

    #[test]
    fn response_stores_recorded_count() {
        let response = AnalyticsEventBatchResponse {
            recorded: 5,
            events: vec![],
        };

        assert_eq!(response.recorded, 5);
    }

    #[test]
    fn response_stores_events() {
        let events = vec![
            AnalyticsEventCreated {
                id: AnalyticsEventId::new("evt_1"),
                event_type: "page_view".to_string(),
            },
            AnalyticsEventCreated {
                id: AnalyticsEventId::new("evt_2"),
                event_type: "scroll".to_string(),
            },
        ];

        let response = AnalyticsEventBatchResponse {
            recorded: 2,
            events,
        };

        assert_eq!(response.recorded, 2);
        assert_eq!(response.events.len(), 2);
        assert_eq!(response.events[0].id.as_str(), "evt_1");
        assert_eq!(response.events[1].id.as_str(), "evt_2");
    }

    #[test]
    fn response_serializes() {
        let response = AnalyticsEventBatchResponse {
            recorded: 1,
            events: vec![AnalyticsEventCreated {
                id: AnalyticsEventId::new("test"),
                event_type: "page_view".to_string(),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("recorded"));
        assert!(json.contains("events"));
        assert!(json.contains("test"));
    }
}
