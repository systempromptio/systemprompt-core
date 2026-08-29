//! `ClientProfile` — what the wire is allowed to contain for a given client.
//!
//! Both predicates gate what a response carries. Too permissive and a client
//! receives content it cannot parse; too strict and a capable client is
//! quietly downgraded. The version threshold is asserted on both sides of the
//! boundary rather than at one point, since an off-by-one there is invisible
//! from a single version.

use std::collections::BTreeSet;

use std::collections::BTreeMap;

use rmcp::model::{ClientCapabilities, Implementation, InitializeRequestParams, ProtocolVersion};
use systemprompt_models::mcp::ClientProfile;

fn profile_with(version: ProtocolVersion, extensions: &[&str]) -> ClientProfile {
    ClientProfile {
        protocol_version: Some(version),
        client_name: Some("test-client".to_owned()),
        extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
    }
}

const UI_EXTENSION: &str = "io.modelcontextprotocol/ui";

// Why: an unidentified client must get the most conservative wire there is.
// Defaulting to "supported" would send UI artifacts and structured content to
// something that announced nothing at all.
#[test]
fn an_unknown_client_supports_nothing() {
    let unknown = ClientProfile::unknown();

    assert!(
        !unknown.supports_ui(),
        "an unidentified client must not be sent UI artifacts"
    );
    assert!(
        !unknown.supports_structured_content(),
        "an unidentified client must not be sent structured content"
    );
    assert!(unknown.protocol_version.is_none());
    assert!(unknown.client_name.is_none());
}

// Why: structured content is gated at 2025-06-18. Asserting only one side of
// the threshold would leave a `>` / `>=` slip undetected, and that slip
// silently downgrades every client on the boundary version.
#[test]
fn structured_content_is_supported_from_its_threshold_version_upward() {
    assert!(
        !profile_with(ProtocolVersion::V_2024_11_05, &[]).supports_structured_content(),
        "2024-11-05 predates structured content and must not be sent it"
    );

    for at_or_above in [
        ProtocolVersion::V_2025_06_18,
        ProtocolVersion::V_2025_11_25,
        ProtocolVersion::V_2026_07_28,
    ] {
        assert!(
            profile_with(at_or_above.clone(), &[]).supports_structured_content(),
            "{at_or_above:?} is at or above the threshold and must be supported"
        );
    }
}

// Why: UI support is announced per client through an extension key, not
// implied by protocol version. A newer client that did not ask for UI must not
// be sent it.
#[test]
fn ui_support_comes_from_the_extension_not_the_version() {
    let newest_without_ui = profile_with(ProtocolVersion::V_2026_07_28, &[]);
    assert!(
        !newest_without_ui.supports_ui(),
        "a recent client that did not announce the UI extension must not receive UI"
    );

    let older_with_ui = profile_with(ProtocolVersion::V_2024_11_05, &[UI_EXTENSION]);
    assert!(
        older_with_ui.supports_ui(),
        "the extension is what grants UI, whatever the protocol version"
    );
}

#[test]
fn an_unrelated_extension_does_not_grant_ui() {
    let other = profile_with(ProtocolVersion::V_2026_07_28, &["io.example/other"]);

    assert!(
        !other.supports_ui(),
        "only the UI extension id grants UI support"
    );
}

// Why: the profile is built once from `initialize` and read for every
// subsequent response. A field dropped here is a capability lost for the whole
// session.
#[test]
fn initialize_params_carry_version_name_and_extensions_into_the_profile() {
    let mut capabilities = ClientCapabilities::default();
    let mut extensions = BTreeMap::new();
    extensions.insert(UI_EXTENSION.to_owned(), serde_json::Map::new());
    capabilities.extensions = Some(extensions);

    let mut client_info = Implementation::default();
    client_info.name = "cowork".to_owned();
    client_info.version = "1.2.3".to_owned();

    let mut params = InitializeRequestParams::default();
    params.protocol_version = ProtocolVersion::V_2025_11_25;
    params.capabilities = capabilities;
    params.client_info = client_info;

    let profile = ClientProfile::from_initialize_params(&params);

    assert_eq!(profile.client_name.as_deref(), Some("cowork"));
    assert_eq!(
        profile.protocol_version,
        Some(ProtocolVersion::V_2025_11_25)
    );
    assert_eq!(
        profile.extensions,
        BTreeSet::from([UI_EXTENSION.to_owned()]),
        "the announced extensions must survive into the profile"
    );
    assert!(
        profile.supports_ui(),
        "the announced UI extension grants UI"
    );
}

// Why: a client that announces no extensions is not an error, but it is also
// not entitled to UI. This is the ordinary case for a plain MCP client.
#[test]
fn a_client_announcing_no_extensions_gets_an_empty_set_rather_than_ui() {
    let mut client_info = Implementation::default();
    client_info.name = "plain".to_owned();
    client_info.version = "1.0.0".to_owned();

    let mut params = InitializeRequestParams::default();
    params.protocol_version = ProtocolVersion::V_2025_06_18;
    params.client_info = client_info;

    let profile = ClientProfile::from_initialize_params(&params);

    assert!(profile.extensions.is_empty());
    assert!(!profile.supports_ui());
    assert!(
        profile.supports_structured_content(),
        "structured content is version-gated, so it is still available"
    );
}
