//! The narrow install views and the conferred capabilities (rulings 1–5): an
//! install reads the config, profile and services views and no other
//! host-sealed seed; it reads a cipher, a share-token signer, a Git source
//! sync handle and a token minter only when it declared the row, each bound
//! to that member; admission refuses a core audience, an audience outside the
//! member's namespace and a core scope; every mint is audited and a failed
//! audit write fails it; and no route set's state reads a view or an issuer.

use std::sync::Arc;
use std::time::Duration;
use systemprompt_test_fixtures::AppContextSeeds as _;

use systemprompt_extension::reserved::{INSTALL_CONFERRALS, INSTALL_SEEDS, InstallConferral};
use systemprompt_extension::{Admitted, ProvideError, Resolved, StandingTag};
use systemprompt_governance::authz::GovernanceDecisionRepository;
use systemprompt_identifiers::{ExtensionId, ManagedResourceId, ManagedSourceId, UserId};
use systemprompt_manifest::profile::{ProfileView, credential_free_url};
use systemprompt_manifest::services::{InstallServicesView, SystemAdmin};
use systemprompt_manifest::{Config, ConfigView};
use systemprompt_models::auth::{
    JwtAudience, ScopedMintError, ScopedTokenMinter, ScopedTokenRequest, ScopedVerifyError,
    TokenAuthority,
};
use systemprompt_runtime::{AppContext, Kernel, KernelError, KernelTokenMinters};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::{
    BackgroundTasks, ConferralIssuer, DynConferralIssuer, GitSourceSync, GitSourceSyncRequest,
    MemberCipher, MemberCipherError, MemberGrant, SecretStore, ShareTokenError, ShareTokenSigner,
    TokenAudienceGrant,
};

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn admitted(probe: Probe) -> Admitted {
    probe_admitted(ComposedParts {
        probes: vec![probe],
        ..ComposedParts::default()
    })
}

fn resolve(ctx: &AppContext, probe: Probe) -> Result<Kernel, KernelError> {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel.resolve(&admitted(probe))?;
    Ok(kernel)
}

fn missing(error: &KernelError, owner: &str, ty: &'static str) -> bool {
    matches!(
        error,
        KernelError::Provide { owner: refused, source: ProvideError::Missing(m), .. }
            if refused.as_str() == owner && m.type_name() == ty
    )
}

#[derive(Clone)]
struct Cipher(Arc<dyn MemberCipher>);

fn cipher_probe(id: &'static str, declare: bool) -> Probe {
    Probe::new(id, move |c| {
        if declare {
            c.conferral(InstallConferral::MemberCipher);
        }
        c.provide(|r: &Resolved<'_>| -> Result<Cipher, ProvideError> {
            Ok(Cipher(r.member_cipher()?))
        });
    })
}

fn cipher_of(ctx: &AppContext, id: &'static str) -> Arc<dyn MemberCipher> {
    resolve(ctx, cipher_probe(id, true))
        .expect("a declared cipher resolves")
        .state()
        .get::<Cipher>()
        .expect("provided")
        .0
        .clone()
}

#[derive(Clone)]
struct Signer(Arc<dyn ShareTokenSigner>);

fn signer_of(ctx: &AppContext, id: &'static str) -> Arc<dyn ShareTokenSigner> {
    resolve(
        ctx,
        Probe::new(id, |c| {
            c.conferral(InstallConferral::ShareTokens);
            c.provide(|r: &Resolved<'_>| -> Result<Signer, ProvideError> {
                Ok(Signer(r.share_tokens()?))
            });
        }),
    )
    .expect("a declared signer resolves")
    .state()
    .get::<Signer>()
    .expect("provided")
    .0
    .clone()
}

#[derive(Clone)]
struct Minter(Arc<dyn ScopedTokenMinter>);

const MINTER: &str = "minter-probe";
const AUDIENCE: &str = "minter-probe:reports";

fn minter_probe(audience: &'static str, scopes: &'static [&'static str]) -> Probe {
    Probe::new(MINTER, move |c| {
        c.token_audience(audience, scopes);
        c.provide(|r: &Resolved<'_>| -> Result<Minter, ProvideError> {
            Ok(Minter(r.token_minter()?))
        });
    })
}

fn minter_of(ctx: &AppContext) -> Arc<dyn ScopedTokenMinter> {
    resolve(ctx, minter_probe(AUDIENCE, &["reports.read"]))
        .expect("a namespaced audience is admitted")
        .state()
        .get::<Minter>()
        .expect("provided")
        .0
        .clone()
}

fn admin_of(ctx: &AppContext) -> UserId {
    ctx.kernel()
        .state()
        .get::<SystemAdmin>()
        .expect("seeded")
        .id()
        .clone()
}

fn mint_request(
    ctx: &AppContext,
    audience: &str,
    scopes: &[&str],
    ttl_secs: i64,
) -> ScopedTokenRequest {
    let _ = ctx;
    ScopedTokenRequest {
        subject: UserId::new("conferred-subject"),
        audience: audience.to_owned(),
        scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        ttl_secs,
    }
}

fn authority(ctx: &AppContext) -> Arc<dyn TokenAuthority> {
    ctx.kernel()
        .state()
        .get::<Arc<dyn TokenAuthority>>()
        .expect("the API seeds its token authority")
        .clone()
}

#[test]
fn the_install_tables_are_exactly_the_ruled_rows() {
    let seeds: Vec<&str> = INSTALL_SEEDS.iter().map(|seed| seed.type_name()).collect();
    assert_eq!(seeds.len(), 4, "{seeds:?}");
    for forbidden in [
        std::any::type_name::<Arc<Config>>(),
        std::any::type_name::<Arc<dyn SecretStore>>(),
        std::any::type_name::<SigningKeys>(),
        std::any::type_name::<Arc<dyn TokenAuthority>>(),
    ] {
        assert!(
            !seeds.contains(&forbidden),
            "{forbidden} is not install-readable"
        );
    }
    for seed in &seeds {
        for marker in [
            "SecretStore",
            "CustomSecrets",
            "SigningKeys",
            "TokenAuthority",
            "Secrets",
        ] {
            assert!(!seed.contains(marker), "{seed} names {marker}");
        }
    }
    let names: Vec<&str> = INSTALL_CONFERRALS.iter().map(|row| row.name()).collect();
    assert_eq!(
        names,
        [
            "member-cipher",
            "share-tokens",
            "git-source-sync",
            "scoped-tokens"
        ]
    );
}

#[test]
fn the_otlp_and_source_locations_carry_no_credential() {
    assert_eq!(
        credential_free_url("https://user:secret@otel.example:4318/v1/traces?api_key=k#frag")
            .as_deref(),
        Some("https://otel.example:4318/v1/traces")
    );
    assert_eq!(credential_free_url("not a url"), None);
}

#[tokio::test]
async fn the_profile_view_drops_the_otlp_headers_and_user_info() {
    let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
    profile.observability.otlp = Some(systemprompt_manifest::profile::OtlpExportConfig {
        endpoint: "https://collector:hunter2@otel.example/ingest?token=t".to_owned(),
        protocol: systemprompt_manifest::profile::OtlpProtocol::Http,
        headers: [("authorization".to_owned(), "Bearer otlp-secret".to_owned())].into(),
        signals: vec![systemprompt_manifest::profile::OtlpSignal::Traces],
        batch_seconds: 15,
    });
    let view = ProfileView::of(&profile);
    let otlp = view.otlp.clone().expect("the target is kept");
    assert_eq!(otlp.endpoint, "https://otel.example/ingest");
    let shown = format!("{view:?}");
    assert!(
        !shown.contains("otlp-secret") && !shown.contains("hunter2"),
        "{shown}"
    );
    assert_eq!(view.name, profile.name);
    assert_eq!(view.bin_path, profile.paths.bin);
}

#[tokio::test]
async fn the_config_view_omits_the_database_urls_and_the_signing_key_path() {
    let ctx = context().await;
    let view = ctx
        .kernel()
        .state()
        .get::<Arc<ConfigView>>()
        .expect("seeded")
        .clone();
    let config = ctx
        .kernel()
        .state()
        .get::<Arc<Config>>()
        .expect("seeded")
        .clone();
    assert_eq!(view.api_external_url, config.api_external_url);
    let shown = format!("{view:?}");
    assert!(
        !shown.contains(&config.database_url),
        "the database URL is not in the view"
    );
    assert!(
        !shown.contains("signing_key_path") && !shown.contains("database_url"),
        "{shown}"
    );
}

#[tokio::test]
async fn no_route_set_reads_a_view_or_an_issuer() {
    let ctx = context().await;
    let kernel = ctx.kernel().as_ref();
    for standing in [StandingTag::Install, StandingTag::Core] {
        let owner = ExtensionId::new("route-owner");
        let route = kernel.route_scoped(&owner, standing, &[]);
        assert!(route.get::<Arc<ConfigView>>().is_err());
        assert!(route.get::<Arc<ProfileView>>().is_err());
        assert!(route.get::<InstallServicesView>().is_err());
        assert!(route.get::<DynConferralIssuer<dyn MemberCipher>>().is_err());
        assert!(route.member_cipher().is_err());
        assert!(route.share_tokens().is_err());
        assert!(route.token_minter().is_err());
        assert!(route.git_source_sync().is_err());
    }
}

#[tokio::test]
async fn an_install_reads_no_issuer_directly() {
    let ctx = context().await;
    let error = resolve(
        &ctx,
        Probe::new("issuer-reader", |c| {
            c.conferral(InstallConferral::MemberCipher);
            c.provide(|r: &Resolved<'_>| -> Result<Cipher, ProvideError> {
                let _ = r.get::<DynConferralIssuer<dyn MemberCipher>>()?;
                Ok(Cipher(r.member_cipher()?))
            });
        }),
    )
    .expect_err("the issuer is sealed");
    assert!(
        missing(
            &error,
            "issuer-reader",
            std::any::type_name::<DynConferralIssuer<dyn MemberCipher>>()
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_install_cannot_provide_an_issuer() {
    let mut kernel = Kernel::detached(
        BackgroundTasks::new(),
        systemprompt_provider_contracts::Dependencies::new(),
    );
    let error = kernel
        .resolve(&admitted(Probe::new("forged-issuer", |c| {
            c.provide(
                |_: &Resolved<'_>| -> Result<DynConferralIssuer<dyn MemberCipher>, ProvideError> {
                    Err(ProvideError::Withheld)
                },
            );
        })))
        .expect_err("the issuer type is the kernel's");
    assert!(
        matches!(error, KernelError::ReservedType { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_member_cipher_opens_only_its_own_ciphertext() {
    let ctx = context().await;
    let first = cipher_of(&ctx, "cipher-first");
    let second = cipher_of(&ctx, "cipher-second");
    let sealed = first.seal(b"connector token").expect("seals");
    assert!(sealed.starts_with(systemprompt_traits::MEMBER_CIPHER_ENVELOPE));
    assert_eq!(
        first.open(&sealed).expect("opens its own"),
        b"connector token"
    );
    assert!(matches!(
        second.open(&sealed),
        Err(MemberCipherError::NotAuthentic)
    ));
    let master = ctx
        .kernel()
        .state()
        .get::<Arc<dyn SecretStore>>()
        .expect("seeded")
        .encryption_master_key()
        .expect("the test secrets hold a master key");
    let core =
        systemprompt_security::at_rest::AtRestCipher::for_member(&master, &ExtensionId::new("ai"))
            .expect("a core member's cipher");
    assert!(
        core.open(&sealed).is_err(),
        "a core member's cipher does not open an install's value"
    );
    assert!(
        first.open(&core.seal("core").expect("seals")).is_err(),
        "an install's conferred cipher does not open a core member's value"
    );
    let bare = sealed
        .strip_prefix(systemprompt_traits::MEMBER_CIPHER_ENVELOPE)
        .expect("enveloped");
    assert!(
        systemprompt_security::at_rest::pre_088::open_hex(&master, bare).is_err(),
        "the master key itself does not open the bare ciphertext"
    );
}

#[tokio::test]
async fn an_undeclared_cipher_is_refused_typed() {
    let ctx = context().await;
    let error = resolve(&ctx, cipher_probe("cipher-undeclared", false))
        .expect_err("the row was not declared");
    assert!(
        missing(
            &error,
            "cipher-undeclared",
            std::any::type_name::<Arc<dyn MemberCipher>>()
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_share_token_verifies_only_for_its_member_and_purpose() {
    let ctx = context().await;
    let first = signer_of(&ctx, "share-first");
    let second = signer_of(&ctx, "share-second");
    let token = first
        .sign("catalog.share", b"plugin-42", Duration::from_secs(60))
        .expect("signs");
    assert_eq!(
        first.verify("catalog.share", &token).expect("verifies"),
        b"plugin-42"
    );
    assert!(matches!(
        first.verify("catalog.other", &token),
        Err(ShareTokenError::NotAuthentic)
    ));
    assert!(matches!(
        second.verify("catalog.share", &token),
        Err(ShareTokenError::NotAuthentic)
    ));
    assert!(matches!(
        first.sign("catalog.share", b"x", Duration::from_secs(366 * 24 * 3600)),
        Err(ShareTokenError::Lifetime)
    ));
    let core = authority(&ctx);
    assert!(
        core.verify_bearer(&token, JwtAudience::FIRST_PARTY)
            .is_err()
    );
    assert!(core.verify_session_cookie(&token).is_err());
    assert!(core.verify_hook_token(&token).is_err());
}

#[derive(Clone)]
struct GitSync(Arc<dyn GitSourceSync>);

#[tokio::test]
async fn the_git_source_sync_is_read_only_when_declared_and_returns_no_credential() {
    let ctx = context().await;
    let declared = resolve(
        &ctx,
        Probe::new("git-declared", |c| {
            c.conferral(InstallConferral::GitSourceSync);
            c.provide_after_governance(|r: &Resolved<'_>| -> Result<GitSync, ProvideError> {
                Ok(GitSync(r.git_source_sync()?))
            });
        }),
    )
    .expect("a declared Git source sync resolves");
    let sync = declared
        .state()
        .get::<GitSync>()
        .expect("provided")
        .0
        .clone();
    let outcome = sync
        .sync(&GitSourceSyncRequest {
            source_id: ManagedSourceId::new("no-such-source"),
            resource_id: ManagedResourceId::new("no-such-resource"),
            upstream_root: "skills".to_owned(),
            upstream_base_revision_id: None,
        })
        .await;
    assert!(outcome.is_err(), "an unknown source is not synced");
    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance_decisions WHERE policy = 'conferred_git_sync' \
         AND actor_id = 'member:git-declared' AND user_id = $1",
    )
    .bind(admin_of(&ctx).as_str())
    .fetch_one(ctx.db_pool().pool().as_ref())
    .await
    .expect("read the audit");
    assert!(
        audited >= 1,
        "every sync is audited, for the system admin, as the member"
    );
    let shown = format!("{sync:?}");
    assert!(
        !shown.contains("Orchestrator") && !shown.contains("Secret"),
        "{shown}"
    );

    let error = resolve(
        &ctx,
        Probe::new("git-undeclared", |c| {
            c.provide_after_governance(|r: &Resolved<'_>| -> Result<GitSync, ProvideError> {
                Ok(GitSync(r.git_source_sync()?))
            });
        }),
    )
    .expect_err("the row was not declared");
    assert!(
        missing(
            &error,
            "git-undeclared",
            std::any::type_name::<Arc<dyn GitSourceSync>>()
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn admission_refuses_a_core_audience_a_foreign_audience_and_a_core_scope() {
    let ctx = context().await;
    for audience in [
        "api",
        "minter-probe:",
        "other-member:reports",
        "mcp",
        "mcp:server",
    ] {
        let error = resolve(&ctx, minter_probe(audience, &["reports.read"]))
            .expect_err("the audience is refused at admission");
        assert!(
            matches!(&error, KernelError::ConferredAudience { owner, .. } if owner.as_str() == MINTER),
            "{audience}: {error:?}"
        );
    }
    for (owner, audience) in [
        ("mcp", "mcp:server"),
        ("api", "api:reports"),
        ("content-http", "content-http:x"),
    ] {
        let error = resolve(
            &ctx,
            Probe::new(owner, move |c| {
                c.token_audience(audience, &["reports.read"]);
            }),
        )
        .expect_err("a core namespace is never an audience namespace");
        assert!(
            matches!(&error, KernelError::ConferredAudience { .. }),
            "{owner}: {error:?}"
        );
    }
    let core_scopes: [&'static [&'static str]; 4] =
        [&["admin"], &["ADMIN"], &["hook:govern"], &["user"]];
    for scopes in core_scopes {
        let scope = scopes[0];
        let error = resolve(&ctx, minter_probe(AUDIENCE, scopes))
            .expect_err("a core scope is refused at admission");
        assert!(
            matches!(&error, KernelError::ConferredScope { scope: refused, .. } if refused == scope),
            "{scope}: {error:?}"
        );
    }
}

#[tokio::test]
async fn a_minted_token_carries_no_session_or_device_claim() {
    use base64::Engine as _;
    let ctx = context().await;
    let minted = minter_of(&ctx)
        .mint(&ScopedTokenRequest {
            subject: UserId::new("claims-subject"),
            ..mint_request(&ctx, AUDIENCE, &["reports.read"], 60)
        })
        .await
        .expect("a declared audience and scope mint");
    let payload = minted.token.split('.').nth(1).expect("a JWT payload");
    let claims: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .expect("base64url payload"),
    )
    .expect("a JSON claim set");
    let mut names: Vec<&str> = claims.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["aud", "azp", "exp", "iat", "iss", "jti", "scope", "sub"],
        "no session, device or client-session claim"
    );
}

#[tokio::test]
async fn a_mint_is_bounded_to_the_declared_grant_and_audited() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    assert!(matches!(
        minter
            .mint(&mint_request(&ctx, "minter-probe:other", &[], 60))
            .await,
        Err(ScopedMintError::UndeclaredAudience(_))
    ));
    assert!(matches!(
        minter.mint(&mint_request(&ctx, "api", &[], 60)).await,
        Err(ScopedMintError::UndeclaredAudience(_))
    ));
    assert!(matches!(
        minter
            .mint(&mint_request(&ctx, AUDIENCE, &["admin"], 60))
            .await,
        Err(ScopedMintError::UndeclaredScope { .. })
    ));
    assert!(matches!(
        minter
            .mint(&mint_request(&ctx, AUDIENCE, &["reports.read"], 3601))
            .await,
        Err(ScopedMintError::Lifetime(3601))
    ));
    assert!(
        matches!(
            minter
                .mint(&ScopedTokenRequest {
                    subject: admin_of(&ctx),
                    ..mint_request(&ctx, AUDIENCE, &["reports.read"], 60)
                })
                .await,
            Err(ScopedMintError::ReservedSubject)
        ),
        "the system admin is never the subject"
    );
    let subject = UserId::new("conferred-subject");
    let minted = minter
        .mint(&ScopedTokenRequest {
            subject: subject.clone(),
            ..mint_request(&ctx, AUDIENCE, &["reports.read"], 60)
        })
        .await
        .expect("a declared audience and scope mint");
    let (actor, user): (String, String) =
        sqlx::query_as("SELECT actor_id, user_id FROM governance_decisions WHERE id = $1")
            .bind(format!("mint-{}", minted.jti.as_str()))
            .fetch_one(ctx.db_pool().pool().as_ref())
            .await
            .expect("the mint is audited");
    assert_eq!(actor, format!("member:{MINTER}"), "the member is the actor");
    assert_eq!(user, subject.as_str(), "the subject is recorded beside it");
    let shown = format!("{minter:?}");
    assert!(
        !shown.contains("authority") && !shown.contains("Repository"),
        "{shown}"
    );
    let policy: String =
        sqlx::query_scalar("SELECT policy FROM governance_decisions WHERE id = $1")
            .bind(format!("mint-{}", minted.jti.as_str()))
            .fetch_one(ctx.db_pool().pool().as_ref())
            .await
            .expect("the mint is audited");
    assert_eq!(policy, "conferred_token_mint");
    let core = authority(&ctx);
    assert!(
        core.verify_bearer(&minted.token, JwtAudience::FIRST_PARTY_AND_UI)
            .is_err(),
        "no core verifier accepts the member's audience"
    );
    assert!(core.verify_hook_token(&minted.token).is_err());
}

#[tokio::test]
async fn a_failed_audit_write_fails_the_mint() {
    let ctx = context().await;
    let unreachable = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy("postgres://nobody@127.0.0.1:1/none")
        .expect("a lazy pool");
    let issuer = KernelTokenMinters::new(
        authority(&ctx),
        ctx.kernel()
            .state()
            .get::<systemprompt_security::keys::VerificationKeyring>()
            .expect("seeded")
            .clone(),
        "test-issuer".to_owned(),
        GovernanceDecisionRepository::from_pool(Arc::new(unreachable)),
        admin_of(&ctx),
    );
    let minter = issuer.issue(&MemberGrant {
        member: ExtensionId::new(MINTER),
        token_audiences: Arc::from(vec![TokenAudienceGrant {
            audience: AUDIENCE.to_owned(),
            scopes: vec!["reports.read".to_owned()],
        }]),
    });
    assert!(matches!(
        minter
            .mint(&ScopedTokenRequest {
                subject: UserId::new("conferred-subject"),
                ..mint_request(&ctx, AUDIENCE, &["reports.read"], 60)
            })
            .await,
        Err(ScopedMintError::Audit(_))
    ));
}

fn signed(ctx: &AppContext, audience: &str, azp: &str, scope: &str) -> String {
    let issuer = ctx
        .kernel()
        .state()
        .get::<Arc<ConfigView>>()
        .expect("seeded")
        .jwt_issuer
        .clone();
    let now = chrono::Utc::now().timestamp();
    authority(ctx)
        .sign_claims(&serde_json::json!({
            "iss": issuer,
            "sub": "conferred-subject",
            "aud": [audience],
            "scope": scope,
            "azp": azp,
            "jti": "forged-jti",
            "iat": now,
            "exp": now + 60,
        }))
        .expect("signs")
}

#[tokio::test]
async fn the_minter_verifies_its_own_token_for_its_declared_audience() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    let minted = minter
        .mint(&mint_request(&ctx, AUDIENCE, &["reports.read"], 60))
        .await
        .expect("mints");
    let claims = minter.verify(AUDIENCE, &minted.token).expect("verifies");
    assert_eq!(claims.subject.as_str(), "conferred-subject");
    assert_eq!(claims.audience, AUDIENCE);
    assert_eq!(claims.scopes, vec!["reports.read".to_owned()]);
    assert_eq!(claims.jti, minted.jti);
    assert_eq!(claims.expires_at, minted.expires_at);
}

#[tokio::test]
async fn the_minter_refuses_a_core_audience_and_a_core_token() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    let core = signed(&ctx, "api", MINTER, "reports.read");
    assert!(matches!(
        minter.verify("api", &core),
        Err(ScopedVerifyError::UndeclaredAudience(_))
    ));
    assert!(matches!(
        minter.verify(AUDIENCE, &core),
        Err(ScopedVerifyError::NotAuthentic(_))
    ));
}

#[tokio::test]
async fn the_minter_refuses_another_members_audience_and_token() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    let other = signed(&ctx, "other-probe:reports", "other-probe", "reports.read");
    assert!(matches!(
        minter.verify("other-probe:reports", &other),
        Err(ScopedVerifyError::UndeclaredAudience(_))
    ));
    let impostor = signed(&ctx, AUDIENCE, "other-probe", "reports.read");
    assert!(matches!(
        minter.verify(AUDIENCE, &impostor),
        Err(ScopedVerifyError::OtherMember)
    ));
    let widened = signed(&ctx, AUDIENCE, MINTER, "reports.read reports.write");
    assert!(matches!(
        minter.verify(AUDIENCE, &widened),
        Err(ScopedVerifyError::UndeclaredScope(_))
    ));
}

#[tokio::test]
async fn the_minter_refuses_an_expired_token() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    let minted = minter
        .mint(&mint_request(&ctx, AUDIENCE, &["reports.read"], 1))
        .await
        .expect("mints");
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(matches!(
        minter.verify(AUDIENCE, &minted.token),
        Err(ScopedVerifyError::Expired)
    ));
}

#[tokio::test]
async fn verify_refuses_the_system_admin_subject() {
    let ctx = context().await;
    let minter = minter_of(&ctx);
    let now = chrono::Utc::now().timestamp();
    let forged = ctx
        .kernel()
        .state()
        .get::<Arc<dyn TokenAuthority>>()
        .expect("the API seeds the token authority")
        .sign_claims(&serde_json::json!({
            "iss": ctx.kernel().state().get::<Arc<Config>>().expect("config").jwt_issuer,
            "sub": admin_of(&ctx).as_str(),
            "aud": [AUDIENCE],
            "scope": "reports.read",
            "azp": MINTER,
            "jti": "forged-admin-subject",
            "iat": now,
            "exp": now + 60,
        }))
        .expect("the authority signs");
    assert!(matches!(
        minter.verify(AUDIENCE, &forged),
        Err(ScopedVerifyError::ReservedSubject)
    ));
}

#[tokio::test]
async fn a_hop_namespace_is_never_a_member_audience() {
    let ctx = context().await;
    for (owner, audience) in [("owner-hop", "owner-hop:svc"), ("mcp", "mcp:server")] {
        let probe = Probe::new(owner, move |c| {
            c.token_audience(audience, &["x.read"]);
        });
        assert!(
            resolve(&ctx, probe).is_err(),
            "{audience} collides with a core hop audience"
        );
    }
}
