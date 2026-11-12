//! A self-issued ID-JAG is resolved under the trusted issuer named in its
//! `upstream_iss` claim: the principal is namespaced under that issuer, takes
//! its link policy, and is refused when the claim is absent or the issuer is
//! no longer trusted.

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_identifiers::ClientId;
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::TrustedIssuer;
use systemprompt_oauth::services::EnterprisePrincipal;
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TYP;
use systemprompt_oauth_issuance::token_exchange::id_jag_subject::{
    IdJagContext, validate_id_jag_subject,
};
use systemprompt_oauth_issuance::{IssuanceError, JwksCache};
use systemprompt_test_fixtures::{
    fixture_config, test_database_url, test_db_pool, test_signing_keys,
};
use systemprompt_traits::{FederatedLink, LinkPolicy};
use systemprompt_users::{UserRepository, UserService};
use uuid::Uuid;

const CLIENT: &str = "sp_web";
const ISSUER_A: &str = "https://idp-a.test";
const ISSUER_B: &str = "https://idp-b.test";

pub fn upstream_issuer(issuer: &str) -> TrustedIssuer {
    TrustedIssuer {
        issuer: issuer.to_owned(),
        jwks_uri: format!("{issuer}/jwks"),
        audience: "systemprompt".to_owned(),
        typ_allowlist: Vec::new(),
        allowed_client_ids: Vec::new(),
        can_issue_id_jag: true,
        link: LinkPolicy::default(),
    }
}

fn config(issuers: Vec<TrustedIssuer>) -> Config {
    let mut config = fixture_config(&test_database_url());
    config.trusted_issuers = issuers;
    config
}

fn self_issued(config: &Config, sub: &str, email: &str, upstream: Option<&str>) -> String {
    let now = Utc::now().timestamp();
    let mut claims = serde_json::json!({
        "iss": config.jwt_issuer,
        "sub": sub,
        "aud": config.jwt_issuer,
        "client_id": CLIENT,
        "jti": Uuid::new_v4().to_string(),
        "iat": now,
        "exp": now + 300,
        "email": email,
        "email_verified": true,
    });
    if let Some(upstream) = upstream {
        claims["upstream_iss"] = serde_json::json!(upstream);
    }
    let keys = test_signing_keys();
    let key = keys.signing_key();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid().to_owned());
    header.typ = Some(ID_JAG_TYP.to_owned());
    let der = key.private_key().to_pkcs1_der().expect("der");
    encode(&header, &claims, &EncodingKey::from_rsa_der(der.as_bytes())).expect("sign")
}

async fn principal(token: &str, config: &Config) -> Result<EnterprisePrincipal, IssuanceError> {
    let pool = test_db_pool().await;
    let repo = systemprompt_test_fixtures::oauth_fixture_repository(&pool);
    let keys = test_signing_keys();
    validate_id_jag_subject(
        token,
        &ClientId::new(CLIENT),
        IdJagContext {
            repo: &repo,
            keys: &keys,
            global: config,
            jwks: &JwksCache::new(),
        },
    )
    .await
    .map(|identity| identity.principal.expect("an ID-JAG carries a principal"))
}

fn tagged(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn a_self_issued_id_jag_without_an_upstream_issuer_is_refused() {
    let config = config(vec![upstream_issuer(ISSUER_A)]);
    let token = self_issued(&config, "sub", "x@upstream.invalid", None);

    let err = principal(&token, &config).await.expect_err("refused");

    assert!(err.to_string().contains("upstream issuer"), "{err}");
}

#[tokio::test]
async fn a_self_issued_id_jag_naming_an_untrusted_upstream_issuer_is_refused() {
    let config = config(vec![upstream_issuer(ISSUER_A)]);
    let token = self_issued(&config, "sub", "x@upstream.invalid", Some(ISSUER_B));

    let err = principal(&token, &config).await.expect_err("refused");

    assert!(err.to_string().contains("is not trusted"), "{err}");
}

#[tokio::test]
async fn an_upstream_issuer_that_may_no_longer_issue_id_jags_is_refused() {
    let config = config(vec![TrustedIssuer {
        can_issue_id_jag: false,
        ..upstream_issuer(ISSUER_A)
    }]);
    let token = self_issued(&config, "sub", "x@upstream.invalid", Some(ISSUER_A));

    let err = principal(&token, &config).await.expect_err("refused");

    assert!(err.to_string().contains("is not trusted"), "{err}");
}

#[tokio::test]
async fn each_upstream_issuer_contributes_its_own_namespace_and_link_policy() {
    let linking = TrustedIssuer {
        link: LinkPolicy {
            link_by_email: true,
            auto_provision: true,
        },
        ..upstream_issuer(ISSUER_A)
    };
    let config = config(vec![linking, upstream_issuer(ISSUER_B)]);
    let sub = tagged("sub");

    let a = principal(
        &self_issued(&config, &sub, "a@x.invalid", Some(ISSUER_A)),
        &config,
    )
    .await
    .expect("A validates");
    let b = principal(
        &self_issued(&config, &sub, "b@x.invalid", Some(ISSUER_B)),
        &config,
    )
    .await
    .expect("B validates");

    assert_eq!(a.issuer, ISSUER_A);
    assert_eq!(b.issuer, ISSUER_B);
    assert!(
        a.link.link_by_email,
        "issuer A's policy reaches A's principal"
    );
    assert!(
        !b.link.link_by_email,
        "issuer A's policy is not applied to issuer B's assertion"
    );
}

#[tokio::test]
async fn the_same_sub_from_two_upstream_issuers_resolves_to_two_users() {
    let config = config(vec![upstream_issuer(ISSUER_A), upstream_issuer(ISSUER_B)]);
    let pool = test_db_pool().await;
    let users = UserService::new(std::sync::Arc::new(UserRepository::new(&pool)));
    let sub = tagged("shared-sub");
    let mut resolved = Vec::new();
    for (issuer, email) in [
        (ISSUER_A, format!("{}@a.invalid", tagged("a"))),
        (ISSUER_B, format!("{}@b.invalid", tagged("b"))),
    ] {
        let p = principal(&self_issued(&config, &sub, &email, Some(issuer)), &config)
            .await
            .expect("validates");
        let (user, link) = users
            .resolve_federated(&p.issuer, &p.sub, &p.verified_claims(), &p.link)
            .await
            .expect("provisioned");
        assert_eq!(link, FederatedLink::Provisioned);
        resolved.push(user.id);
    }

    assert_ne!(resolved[0], resolved[1], "the issuers' subs never alias");
    for id in &resolved {
        let _ = users.delete(id).await;
    }
}
