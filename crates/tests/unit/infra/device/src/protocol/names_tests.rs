use systemprompt_device::protocol::PubkeyError;
use systemprompt_device::protocol::ids::InstallId;
use systemprompt_device::protocol::names::{
    canonical_session_uuid, device_fingerprint, is_plain_id, is_plain_relative_path,
    is_safe_plugin_segment, key_fingerprint, mcp_server_slug, native_session_id,
    pubkey_fingerprint, session_uuid, session_uuid_string, skill_dir_name,
};
use systemprompt_identifiers::UserId;

#[test]
fn session_uuid_matches_the_other_clients() {
    assert_eq!(
        session_uuid_string("ses_0123456789abcdef"),
        "585a6083-9a91-527d-9414-85ec68884f9a"
    );
    assert_eq!(
        session_uuid_string("ses_harness"),
        "75c44ffb-e578-5900-b3c7-940b29cfa7bd"
    );
    assert_eq!(session_uuid("x").get_version_num(), 5);
    assert_eq!(
        native_session_id("ses_harness").as_str(),
        "75c44ffb-e578-5900-b3c7-940b29cfa7bd"
    );
}

#[test]
fn canonical_session_uuid_refuses_other_spellings() {
    let id = "585a6083-9a91-527d-9414-85ec68884f9a";
    assert!(canonical_session_uuid(id).is_some());
    assert!(canonical_session_uuid(&id.to_uppercase()).is_none());
    assert!(canonical_session_uuid(&id.replace('-', "")).is_none());
    assert!(canonical_session_uuid("ses_abc").is_none());
}

#[test]
fn mcp_slug() {
    assert_eq!(mcp_server_slug("alpha-docs"), "alpha-docs");
    assert_eq!(mcp_server_slug("DynBrain Docs"), "dynbrain-docs");
    assert_eq!(mcp_server_slug("  my__server!!v2 "), "my__server-v2");
    assert_eq!(mcp_server_slug("--x--"), "x");
    assert_eq!(mcp_server_slug("!!!"), "mcp-server");
    assert_eq!(mcp_server_slug("caf\u{e9} bar"), "caf-bar");
}

#[test]
fn skill_dir() {
    assert_eq!(skill_dir_name("alpha_notes"), "alpha-notes");
    assert_eq!(skill_dir_name("Who_Am_I"), "who-am-i");
    assert_eq!(skill_dir_name("__x__y__"), "x-y");
    assert_eq!(skill_dir_name("___"), "");
    let long = format!("{}_{}", "a".repeat(63), "b");
    assert_eq!(skill_dir_name(&long), "a".repeat(63));
    assert_eq!(skill_dir_name(&"c".repeat(80)).len(), 64);
}

#[test]
fn plain_ids() {
    assert!(is_plain_id("alpha_notes"));
    assert!(is_plain_id("a.b-c_9"));
    assert!(is_plain_id(&"a".repeat(64)));
    assert!(!is_plain_id(&"a".repeat(65)));
    assert!(!is_plain_id(""));
    assert!(!is_plain_id("-lead"));
    assert!(!is_plain_id(".hidden"));
    assert!(!is_plain_id("has space"));
    assert!(!is_plain_id("a/b"));
}

#[test]
fn plugin_paths_and_segments() {
    assert!(is_plain_relative_path("skills/alpha-notes/SKILL.md"));
    assert!(is_plain_relative_path(".claude-plugin/plugin.json"));
    assert!(!is_plain_relative_path("../x"));
    assert!(!is_plain_relative_path("a//b"));
    assert!(!is_plain_relative_path("/abs"));
    assert!(!is_plain_relative_path("a\\b"));
    assert!(is_safe_plugin_segment("alpha-tools"));
    assert!(!is_safe_plugin_segment(".x"));
    assert!(!is_safe_plugin_segment("a/b"));
    assert!(!is_safe_plugin_segment("a..b"));
    assert!(!is_safe_plugin_segment(""));
}

#[test]
fn fingerprints() {
    assert_eq!(
        key_fingerprint(&[7u8; 32]),
        "sha256:4bb06f8e4e3a7715d201d573d0aa423762e55dabd61a2c02278fa56cc6d294e0"
    );
    assert_eq!(
        pubkey_fingerprint("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=").unwrap(),
        key_fingerprint(&[7u8; 32])
    );
    assert!(matches!(
        pubkey_fingerprint("AAAA"),
        Err(PubkeyError::Length(3))
    ));
    assert!(matches!(
        pubkey_fingerprint("!!"),
        Err(PubkeyError::Base64(_))
    ));
    assert_eq!(
        device_fingerprint(&InstallId::new("install-1"), &UserId::new("user-1")),
        "296d1df91516312ec3f8e9e0743bff539ed339a1dc8ef5386a09c4170661e548"
    );
}
