use std::str::FromStr;

use systemprompt_models::bridge::ids::{
    IdValidationError, ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId,
    SkillName, ToolName, ToolPolicy,
};

#[test]
fn plugin_id_try_new_accepts_non_empty() {
    let id = PluginId::try_new("my-plugin").unwrap();
    assert_eq!(id.as_str(), "my-plugin");
    assert_eq!(id.to_string(), "my-plugin");
    assert_eq!(AsRef::<str>::as_ref(&id), "my-plugin");
}

#[test]
fn plugin_id_try_new_rejects_empty() {
    let err = PluginId::try_new("").unwrap_err();
    assert!(matches!(err, IdValidationError::Empty { .. }));
    let msg = err.to_string();
    assert!(msg.contains("PluginId"));
    assert!(msg.contains("cannot be empty"));
}

#[test]
fn plugin_id_into_inner_returns_owned_string() {
    let id = PluginId::try_new("p").unwrap();
    assert_eq!(id.into_inner(), "p");
}

#[test]
fn plugin_id_serde_round_trips_as_plain_string() {
    let id = PluginId::try_new("foo").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"foo\"");
    let back: PluginId = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), "foo");
}

#[test]
fn plugin_id_deserialize_rejects_empty() {
    let err = serde_json::from_str::<PluginId>("\"\"").unwrap_err();
    assert!(err.to_string().contains("PluginId"));
}

#[test]
fn plugin_id_from_str_and_try_from() {
    let a = PluginId::from_str("a").unwrap();
    let b = PluginId::try_from("b").unwrap();
    let c = PluginId::try_from("c".to_owned()).unwrap();
    assert_eq!(a.as_str(), "a");
    assert_eq!(b.as_str(), "b");
    assert_eq!(c.as_str(), "c");
    let owned: String = a.into();
    assert_eq!(owned, "a");
}

#[test]
fn plugin_id_ordering_and_hash() {
    use std::collections::BTreeSet;
    let mut set: BTreeSet<PluginId> = BTreeSet::new();
    set.insert(PluginId::try_new("b").unwrap());
    set.insert(PluginId::try_new("a").unwrap());
    let first = set.iter().next().unwrap();
    assert_eq!(first.as_str(), "a");
}

#[test]
fn skill_id_skill_name_managed_mcp_tool_name_smoke() {
    assert!(SkillId::try_new("s").is_ok());
    assert!(SkillName::try_new("n").is_ok());
    assert!(ManagedMcpServerName::try_new("m").is_ok());
    assert!(ToolName::try_new("t").is_ok());
    assert!(SkillId::try_new("").is_err());
    assert!(SkillName::try_new("").is_err());
    assert!(ManagedMcpServerName::try_new("").is_err());
    assert!(ToolName::try_new("").is_err());
}

#[test]
fn sha256_digest_accepts_valid_lowercase_hex() {
    let hex = "0123456789abcdef".repeat(4);
    let d = Sha256Digest::try_new(hex.clone()).unwrap();
    assert_eq!(d.as_str(), hex);
    assert_eq!(d.to_string(), hex);
    assert_eq!(AsRef::<str>::as_ref(&d), hex.as_str());
    let s: String = d.clone().into();
    assert_eq!(s, hex);
    assert_eq!(d.into_inner(), hex);
}

#[test]
fn sha256_digest_rejects_wrong_length() {
    let err = Sha256Digest::try_new("abc").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Sha256Digest"));
    assert!(msg.contains("64"));
}

#[test]
fn sha256_digest_rejects_uppercase_hex() {
    let hex = "ABCDEF".to_owned() + &"0".repeat(58);
    let err = Sha256Digest::try_new(hex).unwrap_err();
    assert!(err.to_string().contains("lowercase"));
}

#[test]
fn sha256_digest_rejects_non_hex() {
    let bad = "z".repeat(64);
    let err = Sha256Digest::try_new(bad).unwrap_err();
    assert!(err.to_string().contains("lowercase"));
}

#[test]
fn sha256_digest_serde_round_trip() {
    let hex = "a".repeat(64);
    let d = Sha256Digest::try_new(hex.clone()).unwrap();
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(json, format!("\"{hex}\""));
    let back: Sha256Digest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), hex);
}

#[test]
fn sha256_digest_deserialize_rejects_invalid() {
    let err = serde_json::from_str::<Sha256Digest>("\"too-short\"").unwrap_err();
    assert!(err.to_string().contains("Sha256Digest"));
}

#[test]
fn sha256_digest_from_str_and_try_from() {
    let hex = "f".repeat(64);
    let a = Sha256Digest::from_str(&hex).unwrap();
    let b = Sha256Digest::try_from(hex.clone()).unwrap();
    let c = Sha256Digest::try_from(hex.as_str()).unwrap();
    assert_eq!(a.as_str(), b.as_str());
    assert_eq!(b.as_str(), c.as_str());
}

#[test]
fn manifest_signature_passthrough_no_validation() {
    let sig = ManifestSignature::new("anything-goes==");
    assert_eq!(sig.as_str(), "anything-goes==");
    assert_eq!(sig.to_string(), "anything-goes==");
    assert_eq!(AsRef::<str>::as_ref(&sig), "anything-goes==");
    let from_str: ManifestSignature = "x".into();
    assert_eq!(from_str.as_str(), "x");
    let from_string: ManifestSignature = "y".to_owned().into();
    assert_eq!(from_string.as_str(), "y");
    assert_eq!(sig.into_inner(), "anything-goes==");
}

#[test]
fn manifest_signature_serde_round_trip() {
    let sig = ManifestSignature::new("base64==");
    let json = serde_json::to_string(&sig).unwrap();
    assert_eq!(json, "\"base64==\"");
    let back: ManifestSignature = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), "base64==");
}

#[test]
fn tool_policy_serde_lowercase() {
    for (variant, lower) in [
        (ToolPolicy::Allow, "allow"),
        (ToolPolicy::Deny, "deny"),
        (ToolPolicy::Prompt, "prompt"),
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{lower}\""));
        let back: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
        assert_eq!(variant.to_string(), lower);
    }
}

#[test]
fn id_validation_error_invalid_constructor() {
    let err = IdValidationError::invalid("Foo", "because reasons");
    assert!(matches!(err, IdValidationError::Invalid { .. }));
    let msg = err.to_string();
    assert!(msg.contains("Foo"));
    assert!(msg.contains("because reasons"));
}

#[test]
fn id_validation_error_empty_constructor() {
    let err = IdValidationError::empty("Bar");
    assert!(matches!(err, IdValidationError::Empty { .. }));
    assert!(err.to_string().contains("Bar"));
}
