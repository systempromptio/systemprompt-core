use http::{HeaderMap, HeaderValue, Uri};
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::services::a2a_server::version::negotiate;
use systemprompt_models::a2a::{PROTOCOL_VERSION, VERSION_HEADER};

use super::schema::assert_error_info_envelope;

fn uri(path: &str) -> Uri {
    path.parse().unwrap()
}

fn headers(version: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(VERSION_HEADER, HeaderValue::from_str(version).unwrap());
    headers
}

fn refusal(headers: &HeaderMap, path: &str) -> serde_json::Value {
    let builder = negotiate(headers, &uri(path)).expect_err("version must be refused");
    serde_json::to_value(builder.build(&RequestId::String("req-1".into()))).unwrap()
}

#[test]
fn server_speaks_a2a_1_0() {
    assert_eq!(PROTOCOL_VERSION, "1.0");
    assert_eq!(VERSION_HEADER, "A2A-Version");
}

#[test]
fn header_naming_1_0_is_accepted() {
    assert!(negotiate(&headers("1.0"), &uri("/api/v1/agents/a")).is_ok());
}

#[test]
fn header_name_is_case_insensitive() {
    let mut map = HeaderMap::new();
    map.insert("a2a-version", HeaderValue::from_static("1.0"));
    assert!(negotiate(&map, &uri("/")).is_ok());
}

#[test]
fn query_parameter_is_accepted() {
    assert!(negotiate(&HeaderMap::new(), &uri("/a?A2A-Version=1.0")).is_ok());
    assert!(negotiate(&HeaderMap::new(), &uri("/a?x=1&A2A-Version=1.0")).is_ok());
}

#[test]
fn absent_version_is_refused_with_32009() {
    let json = refusal(&HeaderMap::new(), "/a");
    assert_error_info_envelope(&json, -32009, "VERSION_NOT_SUPPORTED");
    assert_eq!(json["id"], "req-1");
    let metadata = &json["error"]["data"][0]["metadata"];
    assert_eq!(metadata["requested"], "");
    assert_eq!(metadata["supported"], "1.0");
}

#[test]
fn empty_version_is_refused() {
    let json = refusal(&headers(""), "/a");
    assert_error_info_envelope(&json, -32009, "VERSION_NOT_SUPPORTED");
}

#[test]
fn other_versions_are_refused_and_echoed() {
    for version in ["0.3", "0.3.0", "1.0.1", "2.0", "1"] {
        let json = refusal(&headers(version), "/a");
        assert_error_info_envelope(&json, -32009, "VERSION_NOT_SUPPORTED");
        assert_eq!(json["error"]["data"][0]["metadata"]["requested"], version);
    }
}

#[test]
fn header_wins_over_query() {
    let json = refusal(&headers("0.3"), "/a?A2A-Version=1.0");
    assert_eq!(json["error"]["data"][0]["metadata"]["requested"], "0.3");
}

#[test]
fn refusal_maps_to_http_400() {
    let builder = negotiate(&HeaderMap::new(), &uri("/")).unwrap_err();
    assert_eq!(builder.http_status(), http::StatusCode::BAD_REQUEST);
}
