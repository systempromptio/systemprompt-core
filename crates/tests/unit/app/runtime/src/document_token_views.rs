use chrono::Duration;
use systemprompt_documents::download::{DownloadTarget, Rendition, signed_download_url};
use systemprompt_documents::tokens::{DocumentDriveTokens, DocumentLinkTokens};
use systemprompt_documents::upload_links::{UploadGrant, UploadLinkTarget, signed_upload_links};
use systemprompt_identifiers::{
    AccessTokenId, AgentName, DocumentId, DocumentRevisionId, DocumentUploadId, SessionId, UserId,
};
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission, UserJwtRequest};
use systemprompt_traits::{DriveTokenError, DriveTokens};

const ISSUER: &str = "token-views";

fn user() -> AuthenticatedUser {
    AuthenticatedUser::new(
        UserId::new("view-owner"),
        "view-owner".to_owned(),
        "view-owner@example.invalid".to_owned(),
        vec![Permission::User],
    )
}

fn links() -> DocumentLinkTokens {
    DocumentLinkTokens::new(Some(systemprompt_test_fixtures::test_token_authority(
        ISSUER,
    )))
}

fn download_token(view: &DocumentLinkTokens) -> String {
    let (url, _) = signed_download_url(
        view,
        DownloadTarget {
            rendition: Rendition::Native,
            base_url: "https://views.example.test",
            user: &user(),
            session_id: &SessionId::generate(),
            document: &DocumentId::generate(),
            revision: &DocumentRevisionId::generate(),
            agent: &AgentName::new("agent"),
        },
    )
    .expect("the link view signs a download link");
    url.rsplit_once("access_token=")
        .expect("token query")
        .1
        .to_owned()
}

#[test]
fn the_link_view_verifies_each_link_only_against_its_own_audience() {
    let view = links();
    let download = download_token(&view);
    assert!(view.verify_download(&download).is_ok());
    assert!(view.verify_upload(&download).is_err());

    let upload = signed_upload_links(
        &view,
        UploadLinkTarget {
            grant: UploadGrant::Write,
            base_url: "https://views.example.test",
            user: &user(),
            session_id: &SessionId::generate(),
            upload: &DocumentUploadId::generate(),
        },
    )
    .expect("the link view signs an upload link");
    assert!(view.verify_upload(&upload.token).is_ok());
    assert!(view.verify_download(&upload.token).is_err());
}

#[test]
fn the_link_view_admits_no_first_party_token() {
    let authority = systemprompt_test_fixtures::test_token_authority(ISSUER);
    let token = authority
        .mint_user_jwt(&UserJwtRequest {
            user: user(),
            session_id: SessionId::generate(),
            jti: AccessTokenId::generate(),
            permissions: vec![Permission::User],
            audience: vec![JwtAudience::Api],
            ttl: Duration::minutes(5),
            client_id: None,
            purpose: None,
            plugin_id: None,
            interaction: None,
            browser_session: false,
            ui_session: false,
            auth_time: None,
        })
        .expect("the full authority mints an API token");
    let view = DocumentLinkTokens::new(Some(authority));
    assert!(view.verify_download(&token).is_err());
    assert!(view.verify_upload(&token).is_err());
}

#[test]
fn a_view_without_a_token_authority_signs_nothing() {
    let view = DocumentLinkTokens::new(None);
    let refused = signed_upload_links(
        &view,
        UploadLinkTarget {
            grant: UploadGrant::Write,
            base_url: "https://views.example.test",
            user: &user(),
            session_id: &SessionId::generate(),
            upload: &DocumentUploadId::generate(),
        },
    );
    assert!(refused.is_err());
}

#[tokio::test]
async fn the_drive_view_without_an_authority_or_authenticator_trades_and_admits_nothing() {
    let view = DocumentDriveTokens::new(None, None);
    assert!(matches!(
        view.trade("anything").await,
        Err(DriveTokenError::Mint(_))
    ));
    assert!(matches!(
        view.verify("anything").await,
        Err(DriveTokenError::Unverified(_))
    ));
    let signing = DocumentDriveTokens::new(
        Some(systemprompt_test_fixtures::test_token_authority(ISSUER)),
        None,
    );
    assert!(matches!(
        signing.trade(&download_token(&links())).await,
        Err(DriveTokenError::Unverified(_))
    ));
}
