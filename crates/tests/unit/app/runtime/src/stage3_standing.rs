//! Standing in provide, slot and governance resolution (Stage 3 contract
//! amendments 210–214): a member with install standing reads only the
//! host-sealed seeds `reserved::INSTALL_SEEDS` lists and none of another
//! member's sealed provides, in its provide fns, slot entries and governance
//! factories alike, so a forged install asking for the messaging dispatcher,
//! the spawner or the signing keys fails the resolution typed; a core member
//! is not narrowed; and no route set reads a host-sealed seed.

use std::sync::Arc;

use systemprompt_extension::reserved::{INSTALL_SEEDS, InstallSeed};
use systemprompt_extension::{
    FactoryError, GatewayRequestGuard, LateRead, MemberResolved, MemberScope, MissingDependency,
    ProvideError, Resolved, SlotError, StandingTag,
};
use systemprompt_files::contract::AgentAccessSlot;
use systemprompt_files::documents::agents::AgentAccessPolicy;
use systemprompt_identifiers::{ExtensionId, InstanceId};
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_runtime::{AppContext, Kernel, KernelError};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, fixture_admitted_of, probe_admitted,
    test_database_url,
};
use systemprompt_traits::{BackgroundTasks, DynInventoryPublisher, MessagingDispatcher};

type MessagingCell = LateRead<dyn MessagingDispatcher>;

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn reached() -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other("the sealed value was served"))
}

fn resolve_over(
    ctx: &AppContext,
    admitted: &systemprompt_extension::Admitted,
) -> Result<Kernel, KernelError> {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel.resolve(admitted)?;
    Ok(kernel)
}

fn install(probe: Probe) -> systemprompt_extension::Admitted {
    probe_admitted(ComposedParts {
        probes: vec![probe],
        ..ComposedParts::default()
    })
}

fn install_provide_reading<T: Clone + Send + Sync + 'static>(id: &'static str) -> Probe {
    Probe::new(id, |c| {
        c.provide(|r: &Resolved<'_>| -> Result<Launder<T>, ProvideError> {
            Ok(Launder(r.get::<T>()?))
        });
    })
}

#[derive(Clone)]
struct Launder<T>(T);

fn refused_provide(error: &KernelError, owner: &str, missing: &'static str) -> bool {
    matches!(
        error,
        KernelError::Provide { owner: refused, source: ProvideError::Missing(m), .. }
            if refused.as_str() == owner && m.type_name() == missing
    )
}

async fn assert_install_provide_refused<T: Clone + Send + Sync + 'static>(ctx: &AppContext) {
    let error = resolve_over(
        ctx,
        &install(install_provide_reading::<T>("forged-install")),
    )
    .expect_err("an install provide reading a sealed value is refused");
    assert!(
        refused_provide(&error, "forged-install", std::any::type_name::<T>()),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn a_forged_install_provide_asking_for_the_messaging_dispatcher_fails_typed() {
    let ctx = context().await;
    assert_install_provide_refused::<MessagingCell>(&ctx).await;
}

#[tokio::test]
async fn a_forged_install_provide_asking_for_the_spawner_fails_typed() {
    let ctx = context().await;
    assert_install_provide_refused::<Arc<Spawner>>(&ctx).await;
}

#[tokio::test]
async fn a_forged_install_provide_asking_for_the_signing_keys_fails_typed() {
    let ctx = context().await;
    assert_install_provide_refused::<SigningKeys>(&ctx).await;
}

#[tokio::test]
async fn a_forged_install_provide_asking_for_the_token_authority_fails_typed() {
    let ctx = context().await;
    assert!(
        ctx.kernel()
            .state()
            .conferral_of::<Arc<dyn TokenAuthority>>()
            .is_some()
    );
    assert_install_provide_refused::<Arc<dyn TokenAuthority>>(&ctx).await;
}

#[tokio::test]
async fn an_install_provide_reading_another_members_sealed_provide_fails_typed() {
    let ctx = context().await;
    assert!(ctx.kernel().state().is_sealed::<DynInventoryPublisher>());
    assert_install_provide_refused::<DynInventoryPublisher>(&ctx).await;
}

#[tokio::test]
async fn an_install_slot_entry_reading_the_signing_keys_fails_typed() {
    let ctx = context().await;
    let admitted = install(Probe::new("forged-slot", |c| {
        c.slot::<AgentAccessSlot>().policy(
            |r: &Resolved<'_>| -> Result<Arc<dyn AgentAccessPolicy>, FactoryError> {
                r.get::<SigningKeys>()?;
                Err(FactoryError::Failed(reached()))
            },
        );
    }));
    let error = resolve_over(&ctx, &admitted).expect_err("the slot entry is refused");
    assert!(
        matches!(
            &error,
            KernelError::Slot(SlotError::Missing(m))
                if m.type_name() == std::any::type_name::<SigningKeys>()
        ),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn an_install_governance_factory_reading_the_spawner_fails_typed() {
    let ctx = context().await;
    let admitted = install(Probe::new("forged-guard", |c| {
        c.gateway_guard(
            |r: &Resolved<'_>| -> Result<Arc<dyn GatewayRequestGuard>, FactoryError> {
                r.get::<Arc<Spawner>>()?;
                Err(FactoryError::Failed(reached()))
            },
        );
    }));
    let error = resolve_over(&ctx, &admitted).expect_err("the guard factory is refused");
    assert!(
        matches!(
            &error,
            KernelError::GatewayGuard { owner, source: FactoryError::Missing(m) }
                if owner.as_str() == "forged-guard"
                    && m.type_name() == std::any::type_name::<Arc<Spawner>>()
        ),
        "unexpected error: {error:?}"
    );
}

#[derive(Clone)]
struct AllowListed {
    config: Arc<Config>,
    admin: SystemAdmin,
    instance: InstanceId,
}

#[tokio::test]
async fn an_install_provide_reads_every_allow_listed_seed() {
    let ctx = context().await;
    assert_eq!(
        INSTALL_SEEDS,
        &[InstallSeed::Config, InstallSeed::SystemAdmin],
        "a row added to the table is a reviewed decision: extend this case with it"
    );
    for seed in INSTALL_SEEDS {
        assert!(
            ctx.kernel()
                .state()
                .sealed_types()
                .any(|(id, _)| id == seed.type_id()),
            "{} is a host-sealed seed",
            seed.type_name()
        );
    }
    let admitted = install(Probe::new("allow-listed-install", |c| {
        c.provide(|r: &Resolved<'_>| -> Result<AllowListed, ProvideError> {
            Ok(AllowListed {
                config: r.get()?,
                admin: r.get()?,
                instance: r.get()?,
            })
        });
    }));
    let kernel = resolve_over(&ctx, &admitted).expect("the allow-listed reads resolve");
    let read = kernel
        .state()
        .get::<AllowListed>()
        .expect("the install's value is provided");
    assert_eq!(read.config.instance_id, read.instance);
    assert_eq!(
        read.admin.id(),
        ctx.kernel()
            .state()
            .get::<SystemAdmin>()
            .expect("seeded")
            .id()
    );
    assert!(
        ctx.kernel()
            .state()
            .is_sealed::<Arc<dyn systemprompt_traits::SecretStore>>(),
        "the secret store is sealed and not a row: an install reads only its declared secrets"
    );
    assert_install_provide_refused::<Arc<dyn systemprompt_traits::SecretStore>>(&ctx).await;
}

#[derive(Clone)]
struct OwnSealed;

#[derive(Clone)]
struct ReadsOwn;

#[tokio::test]
async fn an_install_provide_reads_its_own_sealed_provide() {
    let ctx = context().await;
    let admitted = install(Probe::new("own-sealed-install", |c| {
        c.provide_sealed(|_: &Resolved<'_>| -> Result<OwnSealed, ProvideError> { Ok(OwnSealed) })
            .provide(|r: &Resolved<'_>| -> Result<ReadsOwn, ProvideError> {
                r.get::<OwnSealed>()?;
                Ok(ReadsOwn)
            });
    }));
    let kernel = resolve_over(&ctx, &admitted).expect("its own sealed value is its own");
    assert!(kernel.state().find::<ReadsOwn>().is_some());
}

#[tokio::test]
async fn a_core_member_provide_still_reads_the_sealed_seeds() {
    let ctx = context().await;
    let admitted = fixture_admitted_of(vec![Box::new(install_provide_reading::<SigningKeys>(
        "core-probe",
    ))]);
    let kernel = resolve_over(&ctx, &admitted).expect("a core provide reads a sealed seed");
    assert!(kernel.state().find::<Launder<SigningKeys>>().is_some());
}

#[tokio::test]
async fn rescoping_an_install_resolution_never_widens_it() {
    let ctx = context().await;
    let kernel = ctx.kernel();
    let install = MemberScope::install(ExtensionId::new("an-install"));
    let core = ctx
        .admitted()
        .built()
        .members()
        .find(|member| member.standing == StandingTag::Core)
        .expect("a core member is admitted")
        .scope();
    let scoped = kernel.member_resolved(&install);
    assert!(scoped.get::<SigningKeys>().is_err());
    let rescoped = MemberResolved::within(&scoped, &core);
    assert_eq!(
        rescoped
            .get::<SigningKeys>()
            .expect_err("still the install's"),
        MissingDependency::of::<SigningKeys>()
    );
    assert!(
        rescoped.get::<Arc<Config>>().is_ok(),
        "an allow-listed seed stays readable"
    );
    let core_resolved = kernel.member_resolved(&core);
    assert!(
        core_resolved.get::<SigningKeys>().is_ok(),
        "core standing reads it"
    );
    assert!(
        MemberResolved::within(&core_resolved, &install)
            .get::<SigningKeys>()
            .is_err(),
        "a core resolution narrows to an install"
    );
}

#[tokio::test]
async fn the_n1_exploit_replayed_fails_at_admission() {
    let ctx = context().await;
    let admitted = install(Probe::new("n1-install", |c| {
        c.provide(
            |r: &Resolved<'_>| -> Result<Launder<MessagingCell>, ProvideError> {
                let dispatcher = r.get::<MessagingCell>()?;
                r.tasks().spawn("n1-capture", {
                    let dispatcher = dispatcher.clone();
                    async move {
                        let _held = dispatcher;
                    }
                });
                Ok(Launder(dispatcher))
            },
        );
    }));
    let error = resolve_over(&ctx, &admitted).expect_err("the capture is refused");
    assert!(
        refused_provide(&error, "n1-install", std::any::type_name::<MessagingCell>()),
        "unexpected error: {error:?}"
    );
}
