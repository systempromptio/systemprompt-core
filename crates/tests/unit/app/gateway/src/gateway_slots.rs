//! T19 (gateway side) and T8's selector order: the gateway's slots refuse a
//! contribution that would shadow a built-in, and consult core entries first.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder, StandingTag,
};
use systemprompt_gateway::contract::{
    OutboundBuilder, OutboundSlot, OverrideSlot, RouteSelectorBuilder, RouteSelectorSlot,
    SafetyScannerBuilder, SafetyScannerSlot,
};
use systemprompt_gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_gateway::{
    NullScanner, OutboundAdapter, RouteSelector, RouteSelectorError, SafetyConfig, SafetyScanner,
    ScannerFactory, ScannerSettings,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_wire::canonical::CanonicalRequest;

use crate::policies::route_selector::{pinned, req, route};

fn resolved_kernel() -> systemprompt_runtime::Kernel {
    systemprompt_test_fixtures::offline_kernel(Dependencies::default())
}

fn entry<B>(owner: &str, standing: StandingTag, builder: B) -> SlotEntry<B> {
    SlotEntry {
        owner: ExtensionId::new(owner),
        standing,
        builder,
    }
}

fn adapter(_: &Resolved<'_>) -> Result<Arc<dyn OutboundAdapter>, FactoryError> {
    Ok(Arc::new(AnthropicOutbound))
}

#[test]
fn every_gateway_slot_is_core_first() {
    assert_eq!(RouteSelectorSlot::ORDER, SlotOrder::CoreFirst);
    assert_eq!(SafetyScannerSlot::ORDER, SlotOrder::CoreFirst);
    assert_eq!(OverrideSlot::ORDER, SlotOrder::CoreFirst);
    assert_eq!(OutboundSlot::ORDER, SlotOrder::CoreFirst);
}

#[test]
fn an_install_adapter_tagged_like_a_builtin_is_refused() {
    let kernel = resolved_kernel();
    for tag in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        let mut builder = OutboundBuilder::default();
        builder.adapter(tag, adapter);
        let err = OutboundSlot::build(
            vec![entry("acme", StandingTag::Install, builder)],
            &kernel.resolved(),
        )
        .expect_err("a built-in tag is refused");
        assert!(
            matches!(&err, SlotError::OutboundShadowsBuiltin { tag: t, owner } if t == tag && owner.as_str() == "acme"),
            "unexpected error: {err:?}"
        );
    }
}

#[test]
fn two_adapters_with_one_tag_are_refused() {
    let kernel = resolved_kernel();
    let mut first = OutboundBuilder::default();
    first.adapter("acme-wire", adapter);
    let mut second = OutboundBuilder::default();
    second.adapter("acme-wire", adapter);
    let err = OutboundSlot::build(
        vec![
            entry("first", StandingTag::Install, first),
            entry("second", StandingTag::Install, second),
        ],
        &kernel.resolved(),
    )
    .expect_err("a duplicate tag is refused");
    assert!(
        matches!(&err, SlotError::DuplicateTag { tag, .. } if tag == "acme-wire"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn a_new_tag_is_added_beside_the_builtins_as_the_negative_control() {
    let kernel = resolved_kernel();
    let mut builder = OutboundBuilder::default();
    builder.adapter("acme-wire", adapter);
    let registry = OutboundSlot::build(
        vec![entry("acme", StandingTag::Install, builder)],
        &kernel.resolved(),
    )
    .expect("a new tag builds");
    assert!(registry.get("acme-wire").is_some());
    assert!(registry.get("anthropic").is_some());
}

fn null_scanner(_: &Resolved<'_>) -> Result<Arc<dyn ScannerFactory>, FactoryError> {
    fn make(_: &SafetyConfig, _: &ScannerSettings) -> Arc<dyn SafetyScanner> {
        Arc::new(NullScanner)
    }
    Ok(Arc::new(make))
}

#[test]
fn a_scanner_named_like_a_builtin_or_twice_is_refused() {
    let kernel = resolved_kernel();
    let mut shadow = SafetyScannerBuilder::default();
    shadow.scanner("heuristic", null_scanner);
    assert!(matches!(
        SafetyScannerSlot::build(
            vec![entry("acme", StandingTag::Install, shadow)],
            &kernel.resolved()
        ),
        Err(SlotError::DuplicateId { .. })
    ));

    let mut first = SafetyScannerBuilder::default();
    first.scanner("secrets", null_scanner);
    let mut second = SafetyScannerBuilder::default();
    second.scanner("secrets", null_scanner);
    assert!(matches!(
        SafetyScannerSlot::build(
            vec![
                entry("first", StandingTag::Install, first),
                entry("second", StandingTag::Install, second),
            ],
            &kernel.resolved()
        ),
        Err(SlotError::DuplicateId { .. })
    ));
}

struct Reroute(&'static str);

#[async_trait]
impl RouteSelector for Reroute {
    fn name(&self) -> &'static str {
        self.0
    }

    async fn refine(
        &self,
        matched: &GatewayRoute,
        _request: &CanonicalRequest,
    ) -> Result<Option<GatewayRoute>, RouteSelectorError> {
        let mut refined = matched.clone();
        refined.deployments = pinned(self.0);
        Ok(Some(refined))
    }
}

#[tokio::test]
async fn the_first_entry_in_core_first_order_is_consulted_first() {
    let kernel = resolved_kernel();
    let mut core = RouteSelectorBuilder::default();
    core.selector(|_: &Resolved<'_>| Ok(Arc::new(Reroute("core")) as Arc<dyn RouteSelector>));
    let mut install = RouteSelectorBuilder::default();
    install.selector(|_: &Resolved<'_>| Ok(Arc::new(Reroute("install")) as Arc<dyn RouteSelector>));
    let chain = RouteSelectorSlot::build(
        vec![
            entry("core", StandingTag::Core, core),
            entry("install", StandingTag::Install, install),
        ],
        &kernel.resolved(),
    )
    .expect("the selectors build");
    let (_, name) = chain
        .refine(&route("claude-*", "anthropic"), &req("claude-opus-4-8"))
        .await
        .expect("a selector re-routes");
    assert_eq!(name, "core");
}
