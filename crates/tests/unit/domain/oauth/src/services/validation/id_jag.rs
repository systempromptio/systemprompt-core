use systemprompt_identifiers::ClientId;
use systemprompt_oauth::services::validation::id_jag::{
    ClaimPolicy, DEFAULT_LEEWAY_SECS, ID_JAG_TYP, IdJagClaims, IdJagError, resolve_bound_resource,
    validate_claims, validate_typ,
};

fn claims() -> IdJagClaims {
    IdJagClaims {
        iss: "https://idp.example".to_owned(),
        sub: "user-1".to_owned(),
        aud: "https://core.example".to_owned(),
        client_id: Some(ClientId::new("client-a")),
        azp: None,
        jti: "jti-1".to_owned(),
        exp: 1_000_000,
        iat: 999_700,
        scope: Some("user".to_owned()),
        email: None,
        resource: None,
    }
}

fn policy(allowed: &[String], now: i64) -> ClaimPolicy<'_> {
    ClaimPolicy {
        expected_audience: "https://core.example",
        authenticated_client: "client-a",
        allowed_client_ids: allowed,
        now,
        leeway: DEFAULT_LEEWAY_SECS,
    }
}

#[test]
fn typ_must_be_id_jag() {
    assert!(validate_typ(Some(ID_JAG_TYP)).is_ok());
    assert_eq!(
        validate_typ(Some("JWT")),
        Err(IdJagError::WrongTyp {
            found: Some("JWT".to_owned())
        })
    );
    assert!(matches!(
        validate_typ(None),
        Err(IdJagError::WrongTyp { .. })
    ));
}

#[test]
fn happy_path() {
    assert!(validate_claims(&claims(), &policy(&[], 999_800)).is_ok());
}

#[test]
fn rejects_wrong_audience() {
    let mut c = claims();
    c.aud = "https://evil.example".to_owned();
    assert!(matches!(
        validate_claims(&c, &policy(&[], 999_800)),
        Err(IdJagError::AudienceMismatch { .. })
    ));
}

#[test]
fn rejects_client_mismatch() {
    let mut c = claims();
    c.client_id = Some(ClientId::new("client-b"));
    assert!(matches!(
        validate_claims(&c, &policy(&[], 999_800)),
        Err(IdJagError::ClientMismatch { .. })
    ));
}

#[test]
fn azp_used_when_client_id_absent() {
    let mut c = claims();
    c.client_id = None;
    c.azp = Some(ClientId::new("client-a"));
    assert!(validate_claims(&c, &policy(&[], 999_800)).is_ok());
}

#[test]
fn enforces_allowed_client_ids() {
    let allowed = vec!["client-a".to_owned()];
    assert!(validate_claims(&claims(), &policy(&allowed, 999_800)).is_ok());
    let allowed_other = vec!["client-z".to_owned()];
    assert!(matches!(
        validate_claims(&claims(), &policy(&allowed_other, 999_800)),
        Err(IdJagError::ClientNotAllowed { .. })
    ));
}

#[test]
fn rejects_expired() {
    assert!(matches!(
        validate_claims(&claims(), &policy(&[], 1_000_200)),
        Err(IdJagError::Expired)
    ));
}

#[test]
fn rejects_future_iat() {
    assert!(matches!(
        validate_claims(&claims(), &policy(&[], 999_000)),
        Err(IdJagError::IssuedInFuture)
    ));
}

#[test]
fn missing_client_binding() {
    let mut c = claims();
    c.client_id = None;
    c.azp = None;
    assert!(matches!(
        validate_claims(&c, &policy(&[], 999_800)),
        Err(IdJagError::MissingClient)
    ));
}

#[test]
fn unbound_id_jag_leaves_the_resource_to_the_request() {
    let c = claims();
    assert_eq!(
        resolve_bound_resource(c.resource.as_deref(), None),
        Ok(None)
    );
    assert_eq!(
        resolve_bound_resource(c.resource.as_deref(), Some("https://a.example")),
        Ok(Some("https://a.example"))
    );
}

#[test]
fn bound_id_jag_pins_the_resource_when_the_request_omits_it() {
    let mut c = claims();
    c.resource = Some("https://a.example".to_owned());
    assert_eq!(
        resolve_bound_resource(c.resource.as_deref(), None),
        Ok(Some("https://a.example"))
    );
    assert_eq!(
        resolve_bound_resource(c.resource.as_deref(), Some("https://a.example")),
        Ok(Some("https://a.example"))
    );
}

#[test]
fn bound_id_jag_refuses_a_different_resource() {
    let mut c = claims();
    c.resource = Some("https://a.example".to_owned());
    assert_eq!(
        resolve_bound_resource(c.resource.as_deref(), Some("https://b.example")),
        Err(IdJagError::ResourceMismatch {
            expected: "https://a.example".to_owned(),
            found: "https://b.example".to_owned(),
        })
    );
}
