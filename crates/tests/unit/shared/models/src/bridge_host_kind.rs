use systemprompt_models::bridge::host::HostKind;

#[test]
fn every_host_round_trips_through_its_wire_id() {
    for kind in HostKind::ALL {
        assert_eq!(kind.as_str().parse::<HostKind>(), Ok(kind));
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.as_str()));
        assert_eq!(serde_json::from_str::<HostKind>(&json).unwrap(), kind);
    }
}

#[test]
fn wire_ids_are_the_bridge_host_ids() {
    let ids: Vec<&str> = HostKind::ALL.map(HostKind::as_str).to_vec();
    assert_eq!(
        ids,
        ["claude-code", "claude-desktop", "codex-cli", "hermes", "opencode"]
    );
}

#[test]
fn an_unknown_or_legacy_alias_is_rejected() {
    assert!("codex".parse::<HostKind>().is_err());
    assert!("open-code".parse::<HostKind>().is_err());
    assert!("".parse::<HostKind>().is_err());
}
