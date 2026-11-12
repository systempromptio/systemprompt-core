use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_ai::{BillingSource, PresentedCredential};
use systemprompt_catalog_fixtures::{CatalogFixture, caller, claude_model, personal, platform};
use systemprompt_gateway::GatewayEngines;
use systemprompt_gateway::service::credentials::{KeyOwner, fill_project, resolve};
use systemprompt_gateway::service::{DispatchError, GatewayError};
use systemprompt_identifiers::{CredentialId, UserId};
use systemprompt_manifest::services::catalog::{
    EndpointConfig, ProviderConfig, ResolvedCatalog, ResolvedHop,
};
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_security::credential::{AuthHeader, AuthScheme};
use systemprompt_test_fixtures::{
    ensure_test_secrets_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_wire::WireProtocol;

fn catalog(
    credential: systemprompt_manifest::services::catalog::CredentialEntry,
) -> ResolvedCatalog {
    CatalogFixture::new()
        .credential("acme", credential)
        .provider("acme", ProviderConfig::new("acme"))
        .endpoint(
            "acme",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://acme.invalid/v1/messages"),
        )
        .model("model-a", claude_model(0))
        .deploy("model-a", ["acme/api"])
        .build()
}

fn hop(catalog: &ResolvedCatalog) -> ResolvedHop {
    catalog.deployments("model-a")[0].clone()
}

struct Rig {
    engines: GatewayEngines,
    owner: KeyOwner,
    cipher: AtRestCipher,
}

async fn rig() -> Rig {
    let pool = test_db_pool().await;
    let secrets = ensure_test_secrets_bootstrap();
    let cipher = AtRestCipher::from_secrets(&secrets.get()).expect("cipher");
    let user: UserId = unique_user_id("gw-cred");
    seed_user_row(&pool, &user, &format!("{}@gw.invalid", user.as_str()))
        .await
        .expect("seed user");
    let engines = GatewayEngines::new(
        Default::default(),
        &systemprompt_test_fixtures::fixture_extension_context(&pool),
        secrets,
    )
    .expect("engines");
    Rig {
        engines,
        owner: KeyOwner {
            credentials: UserProviderCredentialRepository::new(&pool, cipher.clone()),
            user_id: user,
            caller_auth: None,
        },
        cipher,
    }
}

#[tokio::test]
async fn a_personal_hop_without_a_stored_key_is_refused_before_audit() {
    let r = rig().await;
    let catalog = catalog(personal("Acme"));

    let refused = resolve(&catalog, &hop(&catalog), &r.owner, &r.engines)
        .await
        .expect_err("no key");

    match refused {
        DispatchError::PreAudit(GatewayError::PersonalKeyRequired { credential }) => {
            assert_eq!(credential, "acme");
        },
        other => panic!("expected a personal key refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_personal_hop_presents_the_members_key_and_bills_them() {
    let r = rig().await;
    let sealed = r.cipher.seal("sk-member").expect("seal");
    r.owner
        .credentials
        .set_key(&r.owner.user_id, &CredentialId::new("acme"), &sealed, None)
        .await
        .expect("store key");
    let catalog = catalog(personal("Acme"));

    let (call, billing) = resolve(&catalog, &hop(&catalog), &r.owner, &r.engines)
        .await
        .expect("resolved");

    assert_eq!(billing, BillingSource::UserKey);
    assert_eq!(call.endpoint(), "https://acme.invalid/v1/messages");
}

#[tokio::test]
async fn a_platform_hop_whose_secret_is_absent_is_refused_before_audit() {
    let r = rig().await;
    let catalog = catalog(platform("acme_secret_never_provisioned"));

    let refused = resolve(&catalog, &hop(&catalog), &r.owner, &r.engines)
        .await
        .expect_err("no secret");

    assert!(matches!(refused, DispatchError::PreAudit(_)), "{refused:?}");
}

fn caller_catalog() -> ResolvedCatalog {
    CatalogFixture::new()
        .credential("own", caller("Own account"))
        .provider("own", ProviderConfig::new("own"))
        .endpoint(
            "own",
            "api",
            EndpointConfig::new(
                WireProtocol::Anthropic,
                "https://api.anthropic.com/v1/messages",
            ),
        )
        .model("model-a", claude_model(0))
        .deploy("model-a", ["own/api"])
        .build()
}

#[tokio::test]
async fn a_caller_hop_without_the_clients_credential_is_refused_before_audit() {
    let r = rig().await;
    let catalog = caller_catalog();

    let refused = resolve(&catalog, &hop(&catalog), &r.owner, &r.engines)
        .await
        .expect_err("no credential presented");

    match refused {
        DispatchError::PreAudit(GatewayError::CallerCredentialRequired { credential }) => {
            assert_eq!(credential, "own");
        },
        other => panic!("expected a caller credential refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_caller_hop_presents_the_clients_bearer_and_bills_their_own_account() {
    let mut r = rig().await;
    r.owner.caller_auth = Some(PresentedCredential {
        auth: AuthHeader {
            scheme: AuthScheme::Bearer,
            value: "client-credential".to_owned(),
        },
        headers: vec![
            ("user-agent".to_owned(), "client/1.0".to_owned()),
            (
                "anthropic-beta".to_owned(),
                "capability-a,capability-b".to_owned(),
            ),
        ],
    });
    let catalog = caller_catalog();

    let (call, billing) = resolve(&catalog, &hop(&catalog), &r.owner, &r.engines)
        .await
        .expect("resolved");

    assert_eq!(billing, BillingSource::UserSubscription);
    let sent = call.headers_forwarding(
        WireProtocol::Anthropic,
        &[("x-added-by-the-gateway".to_owned(), "1".to_owned())],
    );
    assert_eq!(
        sent,
        vec![
            (
                "authorization".to_owned(),
                "Bearer client-credential".to_owned()
            ),
            ("content-type".to_owned(), "application/json".to_owned()),
            ("anthropic-version".to_owned(), "2023-06-01".to_owned()),
            ("user-agent".to_owned(), "client/1.0".to_owned()),
            (
                "anthropic-beta".to_owned(),
                "capability-a,capability-b".to_owned()
            ),
        ],
        "the client's headers whole, and nothing the gateway would add"
    );
}

#[test]
fn a_project_placeholder_is_filled_or_refused() {
    assert_eq!(
        fill_project("https://x/projects/{project}/models", Some("p-1")).expect("filled"),
        "https://x/projects/p-1/models"
    );
    assert!(fill_project("https://x/projects/{project}/models", None).is_err());
    assert!(fill_project("https://x/projects/{project}/models", Some("")).is_err());
    assert_eq!(
        fill_project("https://x/static", None).expect("no placeholder"),
        "https://x/static"
    );
}
