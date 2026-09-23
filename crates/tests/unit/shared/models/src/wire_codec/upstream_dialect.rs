//! The (wire, hosting) envelope: one table the gateway and the in-process AI
//! service both read. A platform either can reach is pinned here once.

use serde_json::{Map, Value, json};
use systemprompt_models::services::{Hosting, WireProtocol};
use systemprompt_models::wire::upstream::{UpstreamDialect, VERTEX_ANTHROPIC_VERSION};

const VERTEX_ENDPOINT: &str =
    "https://aiplatform.googleapis.com/v1/projects/p/locations/global/publishers/anthropic";

#[test]
fn hosting_is_read_off_the_endpoint_host() {
    assert_eq!(Hosting::of(VERTEX_ENDPOINT), Hosting::Vertex);
    assert_eq!(
        Hosting::of("https://us-east5-aiplatform.googleapis.com/v1/projects/p"),
        Hosting::Vertex
    );
    assert_eq!(
        Hosting::of("https://api.anthropic.com/v1"),
        Hosting::FirstParty
    );
    assert_eq!(
        Hosting::of("https://evil-aiplatform.googleapis.com.example.com/v1"),
        Hosting::FirstParty
    );
    assert_eq!(Hosting::of("not a url"), Hosting::FirstParty);
}

#[test]
fn anthropic_path_depends_on_hosting_and_stream() {
    let first = UpstreamDialect::new(WireProtocol::Anthropic, Hosting::FirstParty);
    let vertex = UpstreamDialect::of(WireProtocol::Anthropic, VERTEX_ENDPOINT);
    assert_eq!(first.path("claude-sonnet-5", true), "/messages");
    assert_eq!(first.path("claude-sonnet-5", false), "/messages");
    assert_eq!(
        vertex.path("claude-sonnet-5", false),
        "/models/claude-sonnet-5:rawPredict"
    );
    assert_eq!(
        vertex.url(&format!("{VERTEX_ENDPOINT}/"), "claude-sonnet-5", true),
        format!("{VERTEX_ENDPOINT}/models/claude-sonnet-5:streamRawPredict")
    );
}

#[test]
fn other_wires_keep_their_paths_on_either_hosting() {
    for hosting in [Hosting::FirstParty, Hosting::Vertex] {
        let chat = UpstreamDialect::new(WireProtocol::OpenAiChat, hosting);
        let responses = UpstreamDialect::new(WireProtocol::OpenAiResponses, hosting);
        let gemini = UpstreamDialect::new(WireProtocol::Gemini, hosting);
        assert_eq!(chat.path("m", true), "/chat/completions");
        assert_eq!(responses.path("m", false), "/responses");
        assert_eq!(gemini.path("m", false), "/models/m:generateContent");
        assert_eq!(
            gemini.path("m", true),
            "/models/m:streamGenerateContent?alt=sse"
        );
    }
}

#[test]
fn api_key_header_is_the_wire_s_own() {
    let dialect = |wire| UpstreamDialect::new(wire, Hosting::FirstParty);
    assert_eq!(
        dialect(WireProtocol::Anthropic).api_key_header(),
        Some("x-api-key")
    );
    assert_eq!(
        dialect(WireProtocol::Gemini).api_key_header(),
        Some("x-goog-api-key")
    );
    assert_eq!(dialect(WireProtocol::OpenAiChat).api_key_header(), None);
    assert_eq!(
        dialect(WireProtocol::OpenAiResponses).api_key_header(),
        None
    );
}

#[test]
fn version_travels_as_a_header_first_party_and_in_the_body_on_vertex() {
    let first = UpstreamDialect::new(WireProtocol::Anthropic, Hosting::FirstParty);
    let vertex = UpstreamDialect::new(WireProtocol::Anthropic, Hosting::Vertex);
    assert_eq!(
        first.required_headers(),
        vec![("anthropic-version", "2023-06-01")]
    );
    assert!(vertex.required_headers().is_empty());
    assert!(vertex.drops_forwarded_header("Anthropic-Version"));
    assert!(!vertex.drops_forwarded_header("anthropic-beta"));
    assert!(!first.drops_forwarded_header("anthropic-version"));
}

#[test]
fn vertex_body_names_no_model_and_pins_the_vertex_version() {
    let body = || -> Map<String, Value> {
        json!({ "model": "claude-sonnet-5", "max_tokens": 8, "stream": true })
            .as_object()
            .cloned()
            .unwrap()
    };

    let mut vertex = body();
    UpstreamDialect::new(WireProtocol::Anthropic, Hosting::Vertex).finish_body(&mut vertex);
    assert!(!vertex.contains_key("model"));
    assert_eq!(vertex["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
    assert_eq!(vertex["stream"], true);

    let mut first = body();
    UpstreamDialect::new(WireProtocol::Anthropic, Hosting::FirstParty).finish_body(&mut first);
    assert_eq!(first, body());

    let mut gemini = body();
    UpstreamDialect::new(WireProtocol::Gemini, Hosting::Vertex).finish_body(&mut gemini);
    assert_eq!(gemini, body());
}
