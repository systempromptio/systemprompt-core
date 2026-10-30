use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use systemprompt_identifiers::{Actor, ClientId, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, TokenAuthority, TokenResult, UserJwtRequest, UserType,
};
use systemprompt_models::documents::DRIVE_SYNC_AUDIENCE;
use systemprompt_oauth_issuance::{DriveSyncError, DriveSyncTokenService};
use systemprompt_security::JwtUserContext;
use systemprompt_traits::AuthUser;

#[derive(Debug, Default)]
struct RecordingAuthority {
    minted: Mutex<Vec<UserJwtRequest>>,
}

impl TokenAuthority for RecordingAuthority {
    fn verify_session_cookie(&self, _token: &str) -> TokenResult<JwtClaims> {
        unreachable!("the exchange never verifies")
    }

    fn verify_bearer(&self, _token: &str, _audiences: &[JwtAudience]) -> TokenResult<JwtClaims> {
        unreachable!("the exchange never verifies")
    }

    fn verify_hook_token(&self, _token: &str) -> TokenResult<JwtClaims> {
        unreachable!("the exchange never verifies")
    }

    fn sign_claims(&self, _claims: &serde_json::Value) -> TokenResult<String> {
        unreachable!("the exchange signs through mint_user_jwt")
    }

    fn mint_user_jwt(&self, req: &UserJwtRequest) -> TokenResult<String> {
        self.minted.lock().expect("lock").push(req.clone());
        Ok("minted".to_owned())
    }
}

fn service() -> (DriveSyncTokenService, Arc<RecordingAuthority>) {
    let authority = Arc::new(RecordingAuthority::default());
    (DriveSyncTokenService::new(authority.clone()), authority)
}

fn device(expires_in_secs: i64) -> JwtUserContext {
    JwtUserContext {
        user_id: UserId::new("drive-user"),
        session_id: SessionId::new("drive-session"),
        role: Permission::User,
        user_type: UserType::User,
        client_id: Some(ClientId::bridge()),
        act_chain: Vec::new(),
        attributes: BTreeMap::new(),
        jti: None,
        exp: chrono::Utc::now().timestamp() + expires_in_secs,
        purpose: None,
        interaction: None,
        browser_session: false,
    }
}

fn account(roles: &[&str]) -> AuthUser {
    AuthUser {
        id: UserId::new("drive-user"),
        name: "drive".to_owned(),
        email: "drive@example.test".to_owned(),
        roles: roles.iter().map(|role| (*role).to_owned()).collect(),
        is_active: true,
        is_service_account: false,
    }
}

#[test]
fn the_device_token_trades_for_a_user_only_drive_token_no_longer_than_itself() {
    let (service, authority) = service();
    let issued = service
        .exchange(&device(600), account(&["admin"]))
        .expect("exchange");

    let minted = authority.minted.lock().expect("lock");
    let request = minted.first().expect("one mint");
    assert_eq!(request.permissions, vec![Permission::User]);
    assert_eq!(
        request.audience,
        vec![JwtAudience::Resource(DRIVE_SYNC_AUDIENCE.to_owned())]
    );
    assert!(request.ttl.num_seconds() <= 600);
    assert!(request.purpose.is_none());
    assert_eq!(issued.token, "minted");
}

#[test]
fn the_drive_token_lives_at_most_an_hour() {
    let (service, authority) = service();
    service
        .exchange(&device(86_400), account(&["user"]))
        .expect("exchange");
    let minted = authority.minted.lock().expect("lock");
    assert_eq!(minted[0].ttl.num_seconds(), 3600);
}

#[test]
fn an_expired_device_token_does_not_trade() {
    let (service, authority) = service();
    let refused = service.exchange(&device(-5), account(&["user"]));
    assert!(
        matches!(refused, Err(DriveSyncError::Expired)),
        "{refused:?}"
    );
    assert!(authority.minted.lock().expect("lock").is_empty());
}

#[test]
fn only_the_bridge_clients_undelegated_unpurposed_token_trades() {
    let (service, authority) = service();
    let mut other_client = device(600);
    other_client.client_id = Some(ClientId::new("sp_other"));
    let mut purposed = device(600);
    purposed.purpose = Some("document_upload:x".to_owned());
    let mut delegated = device(600);
    delegated.act_chain = vec![Actor::user(UserId::new("delegate"))];

    for token in [other_client, purposed, delegated] {
        let refused = service.exchange(&token, account(&["user"]));
        assert!(
            matches!(refused, Err(DriveSyncError::NotDeviceToken)),
            "{refused:?}"
        );
    }
    assert!(authority.minted.lock().expect("lock").is_empty());
}
