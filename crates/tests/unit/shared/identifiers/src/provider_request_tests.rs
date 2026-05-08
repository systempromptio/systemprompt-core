use systemprompt_identifiers::ProviderRequestId;

#[test]
fn try_new_accepts_uuid_shape() {
    ProviderRequestId::try_new("2ed31120-c78c-48a4-a80d-9895168a6231").expect("valid");
}

#[test]
fn try_new_accepts_arbitrary_provider_token() {
    ProviderRequestId::try_new("req_abc_123").expect("valid");
}

#[test]
fn try_new_rejects_empty() {
    assert!(ProviderRequestId::try_new("").is_err());
}

#[test]
fn try_new_rejects_oversize() {
    let long = "x".repeat(257);
    assert!(ProviderRequestId::try_new(long).is_err());
}

#[test]
fn try_new_accepts_at_max_length() {
    let max = "x".repeat(256);
    ProviderRequestId::try_new(max).expect("valid");
}
