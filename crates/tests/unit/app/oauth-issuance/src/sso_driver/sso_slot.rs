//! T19 (SSO side): the SSO slot refuses two providers with one id, and the
//! role resolver a sign-in consults is the first entry in core-first order,
//! so a core member's resolver is used even when an install composed first.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{FactoryError, Resolved, Slot, SlotError, SlotOrder, StandingTag};
use systemprompt_identifiers::UserId;
use systemprompt_oauth_issuance::contract::{SsoBuilder, SsoSlot};
use systemprompt_oauth_issuance::sso::RoleResolver;
use systemprompt_test_fixtures::slots::built_slot;
use systemprompt_traits::sso::{SsoError, SsoProvider};

use super::providers::{OPEN, provider};

#[derive(Debug)]
struct Tagged(&'static str);

#[async_trait]
impl RoleResolver for Tagged {
    async fn resolve(
        &self,
        _user_id: &UserId,
        _directory_roles: &[String],
    ) -> Result<Vec<String>, SsoError> {
        Ok(vec![self.0.to_owned()])
    }
}

fn entry(
    owner: &str,
    standing: StandingTag,
    builder: SsoBuilder,
) -> (&str, StandingTag, SsoBuilder) {
    (owner, standing, builder)
}

fn open_provider(builder: &mut SsoBuilder) {
    builder.provider(
        |_: &Resolved<'_>| -> Result<Option<Arc<dyn SsoProvider>>, FactoryError> {
            Ok(Some(Arc::new(provider(OPEN))))
        },
    );
}

fn resolver(builder: &mut SsoBuilder, tag: &'static str) {
    builder.role_resolver(
        move |_: &Resolved<'_>| -> Result<Arc<dyn RoleResolver>, FactoryError> {
            Ok(Arc::new(Tagged(tag)))
        },
    );
}

#[test]
fn the_sso_slot_is_core_first() {
    assert_eq!(SsoSlot::ORDER, SlotOrder::CoreFirst);
}

#[tokio::test]
async fn two_providers_with_one_id_are_refused() {
    let kernel = systemprompt_test_fixtures::offline_kernel(Default::default());
    let mut first = SsoBuilder::default();
    open_provider(&mut first);
    let mut second = SsoBuilder::default();
    open_provider(&mut second);
    let err = built_slot::<SsoSlot>(
        vec![
            entry("first", StandingTag::Install, first),
            entry("second", StandingTag::Install, second),
        ],
        &kernel.resolved(),
    )
    .expect_err("a duplicate provider id is refused");
    assert!(
        matches!(&err, SlotError::DuplicateId { id, .. } if id == OPEN),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn one_provider_builds_as_the_negative_control() {
    let kernel = systemprompt_test_fixtures::offline_kernel(Default::default());
    let mut builder = SsoBuilder::default();
    open_provider(&mut builder);
    let built = built_slot::<SsoSlot>(
        vec![entry("acme", StandingTag::Install, builder)],
        &kernel.resolved(),
    )
    .expect("one provider builds");
    assert_eq!(built.providers().len(), 1);
}

#[tokio::test]
async fn the_core_role_resolver_is_consulted_first() {
    let kernel = systemprompt_test_fixtures::offline_kernel(Default::default());
    let mut core = SsoBuilder::default();
    resolver(&mut core, "core");
    let mut install = SsoBuilder::default();
    resolver(&mut install, "install");
    let built = built_slot::<SsoSlot>(
        vec![
            entry("core", StandingTag::Core, core),
            entry("install", StandingTag::Install, install),
        ],
        &kernel.resolved(),
    )
    .expect("the resolvers build");
    let roles = built
        .roles()
        .resolve(&UserId::new("user"), &[])
        .await
        .expect("the resolver answers");
    assert_eq!(roles, ["core"]);
}

#[tokio::test]
async fn without_a_resolver_the_directory_roles_stand() {
    let kernel = systemprompt_test_fixtures::offline_kernel(Default::default());
    let built = SsoSlot::build(Vec::new(), &kernel.resolved()).expect("an empty slot builds");
    let roles = built
        .roles()
        .resolve(&UserId::new("user"), &["viewer".to_owned()])
        .await
        .expect("the default resolver answers");
    assert_eq!(roles, ["viewer"]);
}
