//! The files and marketplace slots as the reference composition admits them:
//! the `files` member opens the blob holder, agent access and documents
//! slots and fills them with the core holder, codecs and template provider;
//! `managed_resources` opens the marketplace slot. The agent access slot is
//! single-valued (T19), a disabled member's entries never build while an
//! enabled twin's do (the negative control), and the marketplace slot builds
//! core first with every gate kept.

use std::any::TypeId;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt::files::contract::{
    AgentAccess, AgentAccessSlot, BlobHolderSlot, DocumentSeams, DocumentsSlot,
};
use systemprompt::files::documents::agents::{
    AgentAccessPolicy, AgentActor, AgentCap, DocumentContext,
};
use systemprompt::files::documents::repository::BlobHolders;
use systemprompt::marketplace::contract::{
    MarketplaceSeams, MarketplaceSlot, MarketplaceSlotBuilder,
};
use systemprompt::marketplace::managed::{
    GateContext, GateVerdict, PublicationGate, PublicationGateError,
};
use systemprompt::reference;
use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::{
    Admitted, FactoryError, Resolved, Slot, SlotError, SlotOrder, StandingTag,
};
use systemprompt_governance::contract::RoleDirectorySlot;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::slots::built_slot;

use crate::probe::{Counter, Probe, build, kernel, kernel_with, profile};

struct NamedPolicy(&'static str);

#[async_trait]
impl AgentAccessPolicy for NamedPolicy {
    fn name(&self) -> &'static str {
        self.0
    }

    async fn cap(&self, _doc: &DocumentContext, _agent: &AgentActor) -> AgentCap {
        AgentCap::None
    }
}

struct NamedGate(&'static str);

#[async_trait]
impl PublicationGate for NamedGate {
    fn id(&self) -> &'static str {
        self.0
    }

    async fn admit(&self, _ctx: &GateContext<'_>) -> Result<GateVerdict, PublicationGateError> {
        Ok(GateVerdict::Refuse {
            reason: self.0.to_owned(),
        })
    }
}

fn policy_probe(id: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<AgentAccessSlot>().policy(
            move |_: &Resolved<'_>| -> Result<Arc<dyn AgentAccessPolicy>, FactoryError> {
                hits.hit();
                Ok(Arc::new(NamedPolicy(id)))
            },
        );
    })
}

fn slot_owners<S: Slot>(admitted: &Admitted) -> Vec<String> {
    admitted
        .built()
        .members()
        .filter(|member| {
            member
                .sink
                .slots()
                .iter()
                .any(|record| record.slot() == TypeId::of::<S>())
        })
        .map(|member| member.owner.as_str().to_owned())
        .collect()
}

fn built_on<S: Slot>(
    admitted: &Admitted,
    kernel: &Kernel,
) -> Result<Option<Arc<S::Built>>, SlotError>
where
    S::Built: Clone,
{
    let resolved = kernel.resolved();
    let mut inputs = Vec::new();
    for member in admitted.built().members() {
        for record in member.sink.slots() {
            if record.slot() == TypeId::of::<S>() {
                inputs.extend(record.take_entry(member.owner.clone(), member.standing));
            }
        }
    }
    let mut state = Dependencies::default();
    if let Some(value) = build_slot(inputs, &resolved)? {
        value
            .insert_into(&mut state)
            .expect("an empty state holds no slot value");
    }
    Ok(state.find::<Arc<S::Built>>().cloned())
}

fn built<S: Slot>(admitted: &Admitted) -> Result<Option<Arc<S::Built>>, SlotError>
where
    S::Built: Clone,
{
    built_on::<S>(admitted, &kernel())
}

fn governed_kernel() -> Kernel {
    let roles = RoleDirectorySlot::build(Vec::new(), &kernel().resolved())
        .expect("an empty role directory slot builds");
    kernel_with(Dependencies::default().with(Arc::new(roles)))
}

fn entry<B>(owner: &str, standing: StandingTag, builder: B) -> (&str, StandingTag, B) {
    (owner, standing, builder)
}

fn gate(id: &'static str) -> MarketplaceSlotBuilder {
    let mut builder = MarketplaceSlotBuilder::default();
    builder.publication_gate(
        move |_: &Resolved<'_>| -> Result<Arc<dyn PublicationGate>, FactoryError> {
            Ok(Arc::new(NamedGate(id)))
        },
    );
    builder
}

#[test]
fn files_and_managed_resources_open_their_own_slots() {
    let admitted = build(reference()).expect("the reference composition admits");
    assert_eq!(slot_owners::<BlobHolderSlot>(&admitted), ["files"]);
    assert_eq!(slot_owners::<AgentAccessSlot>(&admitted), ["files"]);
    assert_eq!(slot_owners::<DocumentsSlot>(&admitted), ["files"]);
    assert_eq!(
        slot_owners::<MarketplaceSlot>(&admitted),
        ["managed_resources"]
    );
}

#[test]
fn the_files_member_fills_its_slots_with_the_core_seams() {
    let admitted = build(reference()).expect("the reference composition admits");
    let holders: Arc<BlobHolders> = built::<BlobHolderSlot>(&admitted)
        .expect("the blob holder slot builds")
        .expect("the blob holder slot is built");
    assert_eq!(
        holders.names().collect::<Vec<_>>(),
        ["issue_report_attachments"]
    );
    let access: Arc<AgentAccess> = built::<AgentAccessSlot>(&admitted)
        .expect("the agent access slot builds")
        .expect("the agent access slot is built");
    assert_eq!(access.policy().name(), "default");
    let documents: Arc<DocumentSeams> = built::<DocumentsSlot>(&admitted)
        .expect("the documents slot builds")
        .expect("the documents slot is built");
    assert!(!documents.codecs.formats().is_empty());
    assert_eq!(documents.template_providers.len(), 1);
}

#[test]
fn two_agent_access_policies_are_refused() {
    let first = Arc::new(Counter::default());
    let second = Arc::new(Counter::default());
    let admitted = build(
        reference()
            .with(policy_probe("probe-first", &first))
            .with(policy_probe("probe-second", &second)),
    )
    .expect("both probes admit");
    let err = built::<AgentAccessSlot>(&admitted).expect_err("a second policy is refused");
    assert!(
        matches!(err, SlotError::MultipleEntries { count: 2, .. }),
        "unexpected error: {err:?}"
    );
    assert_eq!(first.get() + second.get(), 0, "no policy factory runs");
}

#[test]
fn one_contributed_agent_access_policy_replaces_the_default() {
    let hits = Arc::new(Counter::default());
    let admitted =
        build(reference().with(policy_probe("probe-policy", &hits))).expect("the probe admits");
    let access = built::<AgentAccessSlot>(&admitted)
        .expect("one policy builds")
        .expect("the agent access slot is built");
    assert_eq!(access.policy().name(), "probe-policy");
    assert_eq!(hits.get(), 1);
}

#[test]
fn a_disabled_members_agent_access_policy_is_never_built() {
    let on = Arc::new(Counter::default());
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(policy_probe("probe-on", &on))
        .with(policy_probe("probe-off", &off))
        .declare()
        .expect("both probes declare")
        .admit(&profile(&["probe-off"]).extensions)
        .expect("admission succeeds");
    let access = built::<AgentAccessSlot>(&admitted)
        .expect("the disabled twin is dropped before the build")
        .expect("the agent access slot is built");
    assert_eq!(access.policy().name(), "probe-on");
    assert_eq!(
        off.get(),
        0,
        "a disabled member's policy factory never runs"
    );
    assert_eq!(on.get(), 1, "the enabled member's policy factory runs");
}

#[test]
fn a_disabled_files_member_leaves_no_files_slot() {
    let admitted = reference()
        .declare()
        .expect("the reference composition declares")
        .admit(&profile(&["files"]).extensions)
        .expect("disabling the files member admits");
    assert!(slot_owners::<BlobHolderSlot>(&admitted).is_empty());
    assert!(slot_owners::<AgentAccessSlot>(&admitted).is_empty());
    assert!(slot_owners::<DocumentsSlot>(&admitted).is_empty());
    assert!(
        built::<DocumentsSlot>(&admitted)
            .expect("an empty slot builds nothing")
            .is_none()
    );
}

#[test]
fn a_disabled_managed_resources_member_leaves_no_marketplace_slot() {
    let admitted = reference()
        .declare()
        .expect("the reference composition declares")
        .admit(&profile(&["managed_resources"]).extensions)
        .expect("disabling the managed resources member admits");
    assert!(slot_owners::<MarketplaceSlot>(&admitted).is_empty());
    let enabled = build(reference()).expect("the reference composition admits");
    let seams: Arc<MarketplaceSeams> = built_on::<MarketplaceSlot>(&enabled, &governed_kernel())
        .expect("the marketplace slot builds")
        .expect("the enabled member's marketplace slot is built");
    assert!(seams.gates().is_empty(), "core contributes no gate");
}

#[test]
fn the_marketplace_slot_is_core_first_and_keeps_every_gate() {
    assert_eq!(MarketplaceSlot::ORDER, SlotOrder::CoreFirst);
    let seams = built_slot::<MarketplaceSlot>(
        vec![
            entry("core", StandingTag::Core, gate("core-gate")),
            entry("install", StandingTag::Install, gate("install-gate")),
        ],
        &governed_kernel().resolved(),
    )
    .expect("the marketplace slot builds");
    assert_eq!(
        seams.gates().iter().map(|g| g.id()).collect::<Vec<_>>(),
        ["core-gate", "install-gate"]
    );
}

#[test]
fn an_install_entry_listed_first_still_builds_after_the_core_entry() {
    let seams = built_slot::<MarketplaceSlot>(
        vec![
            entry("install", StandingTag::Install, gate("install-gate")),
            entry("core", StandingTag::Core, gate("core-gate")),
        ],
        &governed_kernel().resolved(),
    )
    .expect("the marketplace slot builds");
    assert_eq!(
        seams.gates().iter().map(|g| g.id()).collect::<Vec<_>>(),
        ["core-gate", "install-gate"]
    );
}

#[test]
fn the_marketplace_slot_refuses_without_the_role_directory() {
    let err = MarketplaceSlot::build(Vec::new(), &kernel().resolved())
        .expect_err("the role directory is required");
    assert!(
        matches!(err, SlotError::Missing(_)),
        "unexpected error: {err:?}"
    );
}
