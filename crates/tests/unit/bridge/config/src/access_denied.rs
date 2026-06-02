use systemprompt_bridge::config::store::ConfigStoreError;

#[test]
fn access_denied_names_hive_and_subkey() {
    let err = ConfigStoreError::AccessDenied {
        hive: "HKLM".to_string(),
        subkey: r"SOFTWARE\Policies\Claude".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("administrator rights required"),
        "expected an actionable message, got: {msg}"
    );
    assert!(msg.contains("HKLM"), "expected the hive named, got: {msg}");
    assert!(
        msg.contains(r"SOFTWARE\Policies\Claude"),
        "expected the subkey named, got: {msg}"
    );
}
