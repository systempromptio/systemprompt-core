use std::sync::Arc;

use systemprompt_identifiers::{
    ConnectionId, ContextId, McpExecutionId, MessageId, SkillId, SkillName, TaskId,
};
use systemprompt_models::RequestScope;
use systemprompt_models::agui::AgUiEventBuilder;
use systemprompt_models::api::{ApiError, ErrorCode, ResponseLinks, ResponseMeta, SingleResponse};
use systemprompt_models::artifacts::chart::ChartDataset;
use systemprompt_models::artifacts::tool_result::{ToolResultArtifact, ToolResultBlock};
use systemprompt_models::artifacts::types::AxisType;
use systemprompt_models::artifacts::{
    Artifact, ArtifactType, ChartSectionData, Column, TableArtifact,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::content_config::ContentRouting;
use systemprompt_models::events::{ContextSummary, SystemEventBuilder, SystemEventType};
use systemprompt_models::origin::{ClientAttestation, ClientKind, OriginParseError};
use systemprompt_models::routing::{ApiCategory, RouteClassifier, RouteType};

const CTX: &str = "00000000-0000-4000-8000-0000000000b2";

fn context_id() -> ContextId {
    ContextId::try_new(CTX).expect("valid ContextId")
}

#[test]
fn request_scope_overwrites_existing_keys_and_preserves_order() {
    let mut scope = RequestScope::new();
    assert!(scope.is_empty());
    scope.insert("org", "a");
    scope.insert("tenant", "t1");
    scope.insert("org", "b");

    assert!(!scope.is_empty());
    assert_eq!(scope.get("org"), Some("b"));
    assert_eq!(scope.get("tenant"), Some("t1"));
    assert_eq!(scope.get("missing"), None);
    let pairs: Vec<(&str, &str)> = scope.iter().collect();
    assert_eq!(pairs, vec![("org", "b"), ("tenant", "t1")]);
}

#[test]
fn tool_result_artifact_builders_and_schema() {
    let artifact = ToolResultArtifact::new("search")
        .with_server("docs")
        .with_error(true)
        .with_blocks(vec![
            ToolResultBlock::Text {
                text: "hi".to_owned(),
            },
            ToolResultBlock::ResourceLink {
                uri: "ui://panel".to_owned(),
                name: "panel".to_owned(),
                mime_type: None,
            },
        ])
        .with_structured_content(Some(serde_json::json!({"k": 1})));

    assert_eq!(artifact.server_name.as_deref(), Some("docs"));
    assert!(artifact.is_error);
    assert!(artifact.has_ui_resource());
    assert_eq!(
        artifact.structured_content,
        Some(serde_json::json!({"k": 1}))
    );
    assert_eq!(artifact.artifact_type(), ArtifactType::ToolResult);

    let schema = artifact.to_schema();
    assert_eq!(schema["x-artifact-type"], "tool_result");
    assert_eq!(
        schema["required"],
        serde_json::json!(["tool_name", "blocks"])
    );

    let plain = ToolResultArtifact::new("x").with_blocks(vec![ToolResultBlock::Text {
        text: "t".to_owned(),
    }]);
    assert!(!plain.has_ui_resource());
}

#[test]
fn table_artifact_title_skill_and_execution_flow_into_response() {
    let table = TableArtifact::new(vec![Column::new(
        "a",
        systemprompt_models::artifacts::types::ColumnType::String,
    )])
    .with_title("Users")
    .with_skill(SkillId::new("skill-1"), SkillName::new("lookup"))
    .with_execution_id(McpExecutionId::new("exec-77"))
    .with_rows(vec![serde_json::json!({"a": "x"})]);

    let response = table.to_response();
    assert_eq!(response["title"], "Users");
    assert_eq!(response["execution_id"], "exec-77");
    assert_eq!(response["count"], 1);
    assert_eq!(table.artifact_type(), ArtifactType::Table);
}

#[test]
fn chart_section_data_axis_builders() {
    let data = ChartSectionData::new(
        "line",
        vec!["mon".to_owned()],
        vec![ChartDataset::new("hits", vec![1.0])],
    )
    .with_axis_labels("day", "count")
    .with_y_axis_type(AxisType::Logarithmic);

    assert_eq!(data.x_axis_label, "day");
    assert_eq!(data.y_axis_label, "count");
    assert_eq!(data.y_axis_type, AxisType::Logarithmic);
}

#[test]
fn client_attestation_strings_labels_and_parse() {
    for tier in ClientAttestation::ALL {
        assert_eq!(ClientAttestation::parse(tier.as_str()), Ok(tier));
        assert!(!tier.label().is_empty());
    }
    assert_eq!(
        ClientAttestation::HostToken.label(),
        "Attested by bridge host token"
    );
    assert_eq!(ClientAttestation::Unknown.as_str(), "unknown");
    assert!(ClientAttestation::HostToken.is_bridge_channel());
    assert!(ClientAttestation::BridgeSecret.is_bridge_channel());
    assert!(!ClientAttestation::Declared.is_bridge_channel());
    assert_eq!(
        ClientAttestation::parse("forged"),
        Err(OriginParseError::Attestation("forged".to_owned()))
    );
}

#[test]
fn client_kind_labels_hosts_and_products() {
    for kind in ClientKind::ALL {
        assert_eq!(ClientKind::parse(kind.as_str()), Ok(kind));
        assert!(!kind.label().is_empty());
        for host in kind.hosts() {
            assert_eq!(ClientKind::from(host), kind);
        }
    }
    assert_eq!(ClientKind::Other.label(), "API client");
    assert!(ClientKind::Pi.product().is_none());
    assert!(ClientKind::Codex.product().is_some());
    assert!(ClientKind::Unknown.hosts().next().is_none());
    assert!(
        HostKind::ALL
            .iter()
            .any(|h| ClientKind::from(*h) != ClientKind::Unknown)
    );
    assert_eq!(
        ClientKind::parse("nope"),
        Err(OriginParseError::ClientKind("nope".to_owned()))
    );
}

#[test]
fn agui_event_timestamps_are_set_for_every_variant() {
    let before = chrono::Utc::now();
    let events = vec![
        AgUiEventBuilder::run_started(context_id(), TaskId::new("t1"), None),
        AgUiEventBuilder::run_error("boom".to_owned(), None),
        AgUiEventBuilder::step_started("s"),
        AgUiEventBuilder::step_finished("s"),
        AgUiEventBuilder::text_message_end(MessageId::new("m1")),
        AgUiEventBuilder::text_message_content(MessageId::new("m1"), "d"),
        AgUiEventBuilder::state_snapshot(serde_json::json!({})),
        AgUiEventBuilder::state_delta(vec![]),
        AgUiEventBuilder::messages_snapshot(vec![]),
    ];
    for event in events {
        assert!(event.timestamp() >= before);
    }
}

#[test]
fn system_events_report_type_and_timestamp() {
    let before = chrono::Utc::now();
    let now = chrono::Utc::now();
    let cases = vec![
        (
            SystemEventBuilder::context_created(context_id(), "n".to_owned()),
            SystemEventType::ContextCreated,
        ),
        (
            SystemEventBuilder::context_updated(context_id(), None),
            SystemEventType::ContextUpdated,
        ),
        (
            SystemEventBuilder::context_deleted(context_id()),
            SystemEventType::ContextDeleted,
        ),
        (
            SystemEventBuilder::contexts_snapshot(vec![ContextSummary {
                context_id: context_id(),
                name: "n".to_owned(),
                created_at: now,
                updated_at: now,
                message_count: 0,
                task_count: 0,
            }]),
            SystemEventType::ContextsSnapshot,
        ),
        (
            SystemEventBuilder::connected(ConnectionId::new("conn-1")),
            SystemEventType::Connected,
        ),
        (SystemEventBuilder::heartbeat(), SystemEventType::Heartbeat),
    ];
    for (event, expected) in cases {
        assert_eq!(event.event_type(), expected);
        assert!(event.timestamp() >= before);
    }
}

#[derive(Debug)]
struct StatusError(u16);

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "status {}", self.0)
    }
}

impl std::error::Error for StatusError {}

impl systemprompt_traits::ExtensionError for StatusError {
    fn code(&self) -> &'static str {
        "EXT"
    }

    fn status(&self) -> http::StatusCode {
        http::StatusCode::from_u16(self.0).expect("valid status")
    }
}

#[test]
fn extension_error_statuses_map_to_error_codes() {
    let cases = [
        (400, ErrorCode::BadRequest),
        (401, ErrorCode::Unauthorized),
        (403, ErrorCode::Forbidden),
        (404, ErrorCode::NotFound),
        (409, ErrorCode::ConflictError),
        (413, ErrorCode::PayloadTooLarge),
        (423, ErrorCode::Locked),
        (422, ErrorCode::ValidationError),
        (503, ErrorCode::ServiceUnavailable),
        (502, ErrorCode::InternalError),
        (418, ErrorCode::BadRequest),
    ];
    for (status, code) in cases {
        assert_eq!(
            ApiError::from_extension(StatusError(status)).code,
            code,
            "status {status}"
        );
    }
}

#[test]
fn single_response_meta_and_links() {
    let links = ResponseLinks {
        self_link: "/a".to_owned(),
        next: None,
        prev: None,
        docs: "/docs".to_owned(),
    };
    let response = SingleResponse::with_meta(5_u8, ResponseMeta::new()).with_links(links);
    let value = serde_json::to_value(&response).expect("serialize");
    assert_eq!(value["data"], 5);
    assert_eq!(value["links"]["self_link"], "/a");
}

struct DocsRouting;

impl ContentRouting for DocsRouting {
    fn is_html_page(&self, path: &str) -> bool {
        path.starts_with("/docs")
    }

    fn determine_source(&self, _path: &str) -> String {
        "documentation".to_owned()
    }
}

#[test]
fn route_classifier_uses_content_routing_when_present() {
    let classifier = RouteClassifier::new(Some(Arc::new(DocsRouting)));
    assert!(format!("{classifier:?}").contains("content_routing: true"));
    assert_eq!(
        classifier.classify("/docs/intro", "GET"),
        RouteType::HtmlContent {
            source: "documentation".to_owned()
        }
    );
    assert_eq!(
        classifier.classify("/elsewhere", "GET"),
        RouteType::NotFound
    );
    assert_eq!(
        classifier.classify("/api/v1/core/oauth/token", "POST"),
        RouteType::ApiEndpoint {
            category: ApiCategory::Core
        }
    );
    assert!(!classifier.is_html("/elsewhere"));
}

#[test]
fn account_kind_round_trips_and_gates_interactive_login() {
    use std::str::FromStr;
    use systemprompt_models::auth::enums::AccountKind;

    assert_eq!(
        AccountKind::from_str("person").expect("person"),
        AccountKind::Person
    );
    assert_eq!(AccountKind::Person.to_string(), "person");
    assert_eq!(AccountKind::Service.to_string(), "service");
    assert!(AccountKind::Person.allows_interactive_login());
    assert!(!AccountKind::Service.allows_interactive_login());
    assert!(AccountKind::from_str("robot").is_err());
}

#[test]
fn behavioral_bot_reasons_parse_their_own_strings() {
    use systemprompt_models::analytics::BehavioralBotReason;

    for reason in BehavioralBotReason::ALL {
        assert_eq!(BehavioralBotReason::parse(reason.as_str()), Some(reason));
        assert_eq!(reason.to_string(), reason.as_str());
    }
    assert_eq!(BehavioralBotReason::parse("not-a-reason"), None);
}

#[test]
fn attribution_enums_display_and_reject_unknowns() {
    use systemprompt_models::attribution::{AttributionSource, TrafficClass};

    assert_eq!(AttributionSource::ApiKey.to_string(), "api_key");
    assert_eq!(AttributionSource::parse("bogus"), None);
    assert_eq!(TrafficClass::Evaluation.to_string(), "evaluation");
    assert_eq!(TrafficClass::parse("bogus"), None);
}

#[test]
fn context_kind_strings_and_display() {
    use systemprompt_models::ContextKind;

    for (kind, s) in [
        (ContextKind::Evaluation, "evaluation"),
        (ContextKind::McpValidation, "mcp_validation"),
        (ContextKind::CliProbe, "cli_probe"),
    ] {
        assert_eq!(kind.as_str(), s);
        assert_eq!(kind.to_string(), s);
    }
}

#[test]
fn cli_paths_name_the_operator_commands() {
    use systemprompt_models::CliPaths;

    assert_eq!(CliPaths::db_migrate_cmd(), "infra db migrate");
    assert_eq!(CliPaths::agent_run_cmd_pattern(), "admin agents run");
}

#[test]
fn a2a_part_media_type_override() {
    use systemprompt_models::a2a::message::Part;

    let part = Part::url("https://x.test/a.png", "image/png").with_media_type("image/webp");
    assert_eq!(part.media_type.as_deref(), Some("image/webp"));
    assert!(part.as_text().is_none());
    assert!(Part::text("t").as_raw().is_none());
}

#[test]
fn cli_artifact_type_strings_titles_and_bodies() {
    use systemprompt_models::artifacts::audio::AudioArtifact;
    use systemprompt_models::artifacts::image::ImageArtifact;
    use systemprompt_models::artifacts::types::ChartType;
    use systemprompt_models::artifacts::video::VideoArtifact;
    use systemprompt_models::artifacts::{ChartArtifact, CliArtifact, CopyPasteTextArtifact};

    let chart = CliArtifact::Chart {
        artifact: ChartArtifact::new("Trend", ChartType::Bar),
    };
    assert_eq!(chart.artifact_type_str(), "chart");
    assert_eq!(chart.title().as_deref(), Some("Trend"));

    let audio = CliArtifact::Audio {
        artifact: AudioArtifact::new("a.mp3"),
    };
    assert_eq!(audio.artifact_type_str(), "audio");
    assert!(audio.title().is_none());

    let image = CliArtifact::Image {
        artifact: ImageArtifact::new("a.png"),
    };
    assert_eq!(image.artifact_type_str(), "image");
    let video = CliArtifact::Video {
        artifact: VideoArtifact::new("a.mp4"),
    };
    assert_eq!(video.artifact_type_str(), "video");
    assert!(video.text_body().is_none());

    let copy = CliArtifact::CopyPasteText {
        artifact: CopyPasteTextArtifact::new("body"),
    };
    assert_eq!(copy.text_body().as_deref(), Some("body"));
}

#[test]
fn context_events_carry_their_inner_timestamp() {
    use systemprompt_models::events::ContextEvent;

    let inner = AgUiEventBuilder::step_started("s");
    let stamp = inner.timestamp();
    let event = ContextEvent::from(inner);
    assert_eq!(event.timestamp(), stamp);
}
