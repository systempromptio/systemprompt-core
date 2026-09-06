//! SEP-2575 `_meta` stamping on outbound MCP requests.
//!
//! From 2026-07-28 a stateless server rejects any non-initialize request whose
//! `_meta` omits the negotiated protocol version and client capabilities. rmcp
//! sets the header but not the `_meta` fields, so this stamping is the only
//! thing standing between the client and a transport-level rejection of every
//! call — and it must stay silent for older servers, which would reject the
//! fields as unexpected.

use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use rmcp::model::{
    ClientCapabilities, ClientJsonRpcMessage, GetMeta, JsonRpcRequest, ProtocolVersion, RequestId,
};
use systemprompt_mcp::test_api::stamp_request_metadata;

const HEADER: &str = "mcp-protocol-version";

fn headers(negotiated: &str) -> HashMap<HeaderName, HeaderValue> {
    let mut map = HashMap::new();
    map.insert(
        HeaderName::from_static(HEADER),
        HeaderValue::from_str(negotiated).expect("header value"),
    );
    map
}

fn tools_list_request() -> ClientJsonRpcMessage {
    let request: rmcp::model::ClientRequest = rmcp::model::ListToolsRequest::default().into();
    ClientJsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: rmcp::model::JsonRpcVersion2_0,
        id: RequestId::Number(1),
        request,
    })
}

fn capabilities() -> ClientCapabilities {
    ClientCapabilities::builder().enable_elicitation().build()
}

fn stamped_meta(message: &ClientJsonRpcMessage) -> Option<(bool, bool)> {
    let ClientJsonRpcMessage::Request(request) = message else {
        return None;
    };
    let meta = request.request.get_meta();
    Some((
        meta.protocol_version().is_some(),
        meta.client_capabilities().is_some(),
    ))
}

// Why: this is the whole point of the module. Without both fields a
// 2026-07-28 server answers "request _meta is missing or has malformed
// required fields" and no tool call ever completes.
#[test]
fn a_request_on_the_negotiated_2026_protocol_is_stamped_with_version_and_capabilities() {
    let mut message = tools_list_request();

    stamp_request_metadata(
        &mut message,
        &headers(ProtocolVersion::V_2026_07_28.as_str()),
        &capabilities(),
    );

    assert_eq!(
        stamped_meta(&message),
        Some((true, true)),
        "both required _meta fields must be present"
    );
}

// Why: the stamped version has to be the one rmcp actually negotiated. A
// mismatch between the header and the `_meta` field is rejected as loudly as
// an omission.
#[test]
fn the_stamped_version_is_the_one_the_header_reports() {
    let mut message = tools_list_request();
    let negotiated = ProtocolVersion::V_2026_07_28;

    stamp_request_metadata(&mut message, &headers(negotiated.as_str()), &capabilities());

    let ClientJsonRpcMessage::Request(request) = &message else {
        panic!("still a request");
    };
    assert_eq!(
        request.request.get_meta().protocol_version(),
        Some(negotiated),
        "the _meta version must agree with the header"
    );
}

// Why: below 2026-07-28 the fields are not part of the contract, and sending
// them to an older or third-party server is a change it did not ask for.
#[test]
fn a_request_below_the_2026_protocol_is_left_untouched() {
    for older in ["2025-03-26", "2025-06-18"] {
        let mut message = tools_list_request();

        stamp_request_metadata(&mut message, &headers(older), &capabilities());

        assert_eq!(
            stamped_meta(&message),
            Some((false, false)),
            "{older} predates the requirement and must not be stamped"
        );
    }
}

// Why: a version this client does not know is one whose `_meta` contract it
// cannot claim to satisfy. Resolving against the SDK's own list rather than
// echoing the string is what keeps it from asserting compliance blindly.
#[test]
fn a_version_the_client_does_not_recognise_is_left_untouched() {
    let mut message = tools_list_request();

    stamp_request_metadata(&mut message, &headers("2099-01-01"), &capabilities());

    assert_eq!(
        stamped_meta(&message),
        Some((false, false)),
        "an unknown future version must not be claimed as satisfied"
    );
}

// Why: the header is how the negotiated version is discovered. Without it
// there is nothing to stamp, and guessing would risk the mismatch rejection.
#[test]
fn a_request_with_no_protocol_header_is_left_untouched() {
    let mut message = tools_list_request();

    stamp_request_metadata(&mut message, &HashMap::new(), &capabilities());

    assert_eq!(
        stamped_meta(&message),
        Some((false, false)),
        "with no negotiated version there is nothing to stamp"
    );
}

// Why: notifications carry no `_meta` contract, and the early return is what
// keeps the stamping from touching a message shape it does not own.
#[test]
fn a_notification_is_not_a_request_and_is_left_alone() {
    let notification: rmcp::model::ClientNotification =
        rmcp::model::InitializedNotification::default().into();
    let mut message = ClientJsonRpcMessage::Notification(rmcp::model::JsonRpcNotification {
        jsonrpc: rmcp::model::JsonRpcVersion2_0,
        notification,
    });

    stamp_request_metadata(
        &mut message,
        &headers(ProtocolVersion::V_2026_07_28.as_str()),
        &capabilities(),
    );

    assert!(
        matches!(message, ClientJsonRpcMessage::Notification(_)),
        "the notification must pass through unchanged"
    );
}
