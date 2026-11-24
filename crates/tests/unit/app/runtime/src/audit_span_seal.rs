//! The audit-span sink is a sealed kernel seed: the core provides that build
//! the MCP execution ledger and the governance decision writer read it, and no install provide, install route set or member route set resolves it,
//! so nothing outside core can emit an `ai_request`, `tool_call` or
//! `governance_decision` span into the export.

use std::sync::Arc;

use systemprompt_extension::{ProvideError, Resolved, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::audit_spans::DynAuditSpanSink;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::BackgroundTasks;

#[derive(Clone)]
struct SinkSeen(bool);

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

#[tokio::test]
async fn the_kernel_seeds_the_audit_span_sink_sealed() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.find::<DynAuditSpanSink>().is_some());
    assert!(state.is_sealed::<DynAuditSpanSink>());
}

#[tokio::test]
async fn an_install_provide_resolves_no_audit_span_sink() {
    let ctx = context().await;
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel
        .resolve(&probe_admitted(ComposedParts {
            probes: vec![Probe::new("fx-span-install", |c| {
                c.provide(|r: &Resolved<'_>| -> Result<SinkSeen, ProvideError> {
                    Ok(SinkSeen(r.optional::<DynAuditSpanSink>().is_some()))
                });
            })],
            ..ComposedParts::default()
        }))
        .expect("the install probe resolves");
    let seen = kernel.state().get::<SinkSeen>().expect("the probe provided");
    assert!(!seen.0, "an install provide read the audit-span sink");
}

#[tokio::test]
async fn no_install_or_member_route_set_resolves_the_audit_span_sink() {
    let ctx = context().await;
    let install = ExtensionId::new("an-install");
    assert!(
        ctx.kernel()
            .route_scoped(&install, StandingTag::Install, &[])
            .optional::<DynAuditSpanSink>()
            .is_none()
    );
    for owner in [
        "gateway-http",
        "identity-http",
        "mcp-http",
        "content-http",
        "marketplace-http",
    ] {
        let owner = ExtensionId::new(owner);
        assert!(
            ctx.kernel()
                .route_scoped(&owner, StandingTag::Core, &[])
                .optional::<DynAuditSpanSink>()
                .is_none(),
            "{owner}"
        );
    }
}
