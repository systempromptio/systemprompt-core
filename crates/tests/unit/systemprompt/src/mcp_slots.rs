//! The MCP, MCP-host and agent-runs capabilities as the reference composition
//! admits them. `mcp` opens the UI and access-recorder slots and `mcp_host`
//! the toolset slot, so each built value exists exactly when its member is
//! admitted; a second theme, two renderers for one artifact type and two
//! toolsets with one id are refused; a hosted toolset id no member
//! contributes refuses the build; a disabled member's contributions are never
//! built while an enabled twin's are (the negative controls). `agent_runs_api`
//! holds one route set in the `Agent` group, served on every role that serves
//! the agent protocol (T1 row), and the admitted run origins are those of the
//! enabled members.

use std::any::TypeId;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt::reference;
use systemprompt_agent_runs::origin::AdmittedRunOrigins;
use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::router::{ApiBase, BasePath, BaseRef};
use systemprompt_extension::{Admitted, FactoryError, Resolved, RouteGroup, Slot, SlotError};
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::services::ServicesView;
use systemprompt_mcp::McpDomainResult;
use systemprompt_mcp::contract::{AccessRecorderSlot, AccessRecorders, UiSlot};
use systemprompt_mcp::host::{McpAccessEvent, McpAccessRecorder, SharedMcpAccessRecorder};
use systemprompt_mcp::services::ui_renderer::{
    ArtifactTheme, UiRenderer, UiRendererRegistry, UiResource,
};
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::contract::{ToolsetSlot, Toolsets, UnregisteredToolsets};
use systemprompt_mcp_host::toolsets::ToolsetBuildResult;
use systemprompt_models::A2aArtifact as Artifact;
use systemprompt_models::artifacts::types::ArtifactType;
use systemprompt_models::mcp::deployment::Deployment;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::{ConfigWatch, RunOrigin};

use crate::probe::{Counter, Probe, build, profile};

const PROBE_TYPE: &str = "probe_artifact";

struct ProbeRenderer;

impl UiRenderer for ProbeRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Custom(PROBE_TYPE.to_owned())
    }

    fn render(&self, _: &Artifact) -> McpDomainResult<UiResource> {
        Ok(UiResource::new("<p>probe</p>".to_owned()))
    }
}

#[derive(Debug)]
struct ProbeRecorder;

#[async_trait]
impl McpAccessRecorder for ProbeRecorder {
    async fn record(&self, _: McpAccessEvent<'_>) {}
}

fn unavailable(_: &HostContext, _: &McpServerId) -> ToolsetBuildResult {
    Ok(None)
}

fn renderer_probe(id: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<UiSlot>().renderer(
            move |_: &Resolved<'_>| -> Result<Arc<dyn UiRenderer>, FactoryError> {
                hits.hit();
                Ok(Arc::new(ProbeRenderer))
            },
        );
    })
}

fn theme_probe(id: &'static str) -> Probe {
    Probe::new(id, |c| {
        c.slot::<UiSlot>()
            .theme(ArtifactTheme::new("--mcpui-accent: #000;"));
    })
}

fn recorder_probe(id: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<AccessRecorderSlot>().recorder(
            move |_: &Resolved<'_>| -> Result<SharedMcpAccessRecorder, FactoryError> {
                hits.hit();
                Ok(Arc::new(ProbeRecorder))
            },
        );
    })
}

fn toolset_probe(id: &'static str, toolset: &'static str) -> Probe {
    Probe::new(id, move |c| {
        c.slot::<ToolsetSlot>().toolset(toolset, unavailable);
    })
}

fn origin_probe(id: &'static str, origin: &'static str) -> Probe {
    Probe::new(id, move |c| {
        c.run_origin(RunOrigin::new(origin));
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

fn built_with<S: Slot>(
    admitted: &Admitted,
    seed: Dependencies,
) -> Result<Option<Arc<S::Built>>, SlotError> {
    let kernel = systemprompt_test_fixtures::offline_kernel(seed);
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

fn built<S: Slot>(admitted: &Admitted) -> Result<Option<Arc<S::Built>>, SlotError> {
    built_with::<S>(admitted, Dependencies::default())
}

fn services_hosting(toolset: &str) -> ServicesView {
    let base = systemprompt_test_fixtures::fixture_services_view();
    let mut config = (*base.config()).clone();
    let deployment: Deployment = serde_json::from_value(serde_json::json!({
        "server_type": "internal",
        "port": 5199,
        "enabled": true,
        "display_in_web": false,
        "oauth": { "required": false, "scopes": [], "audience": "mcp", "client_id": null },
        "toolsets": [{ "id": toolset, "enabled": true }]
    }))
    .expect("a hosted deployment");
    config
        .mcp_servers
        .insert("probe-hosted".to_owned(), deployment);
    ServicesView {
        config: ConfigWatch::new(config),
        catalog: Arc::clone(&base.catalog),
    }
}

#[test]
fn mcp_opens_the_ui_and_access_recorder_slots() {
    let admitted = build(reference()).expect("the reference composition admits");
    assert_eq!(slot_owners::<UiSlot>(&admitted), ["mcp"]);
    assert_eq!(slot_owners::<AccessRecorderSlot>(&admitted), ["mcp"]);

    let ui: Arc<UiRendererRegistry> = built::<UiSlot>(&admitted)
        .expect("the ui slot builds")
        .expect("the mcp member's slot is built");
    assert!(ui.supports("table"), "core's renderers are built in");
    assert!(ui.theme().is_none(), "core contributes no theme");
    let recorders: Arc<AccessRecorders> = built::<AccessRecorderSlot>(&admitted)
        .expect("the recorder slot builds")
        .expect("the mcp member's slot is built");
    assert!(
        recorders.to_vec().is_empty(),
        "core contributes no recorder"
    );
}

#[test]
fn mcp_host_opens_the_toolset_slot() {
    let admitted = build(reference()).expect("the reference composition admits");
    assert_eq!(slot_owners::<ToolsetSlot>(&admitted), ["mcp_host"]);
}

#[test]
fn a_second_theme_is_refused() {
    let admitted = build(
        reference()
            .with(theme_probe("probe-theme-a"))
            .with(theme_probe("probe-theme-b")),
    )
    .expect("both probes admit");
    let error = built::<UiSlot>(&admitted).expect_err("two themes refuse");
    assert!(
        matches!(
            error,
            SlotError::MultipleEntries {
                slot: "mcp_ui",
                count: 2
            }
        ),
        "{error:?}"
    );
}

#[test]
fn one_contributed_theme_is_applied() {
    let admitted = build(reference().with(theme_probe("probe-theme"))).expect("the probe admits");
    let ui = built::<UiSlot>(&admitted)
        .expect("one theme builds")
        .expect("the ui slot is built");
    assert!(ui.theme().is_some());
}

#[test]
fn two_renderers_for_one_artifact_type_are_refused() {
    let first = Arc::new(Counter::default());
    let second = Arc::new(Counter::default());
    let admitted = build(
        reference()
            .with(renderer_probe("probe-render-a", &first))
            .with(renderer_probe("probe-render-b", &second)),
    )
    .expect("both probes admit");
    let error = built::<UiSlot>(&admitted).expect_err("a duplicate artifact type refuses");
    assert!(
        matches!(&error, SlotError::DuplicateTag { slot: "mcp_ui", tag } if tag == PROBE_TYPE),
        "{error:?}"
    );
}

#[test]
fn a_disabled_members_renderer_is_never_built() {
    let on = Arc::new(Counter::default());
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(renderer_probe("probe-render-off", &off))
        .declare()
        .expect("the probe declares")
        .admit(&profile(&["probe-render-off"]))
        .expect("admission succeeds");
    let ui = built::<UiSlot>(&admitted)
        .expect("the ui slot builds")
        .expect("the ui slot is built");
    assert!(!ui.supports(PROBE_TYPE));
    assert_eq!(off.get(), 0, "a disabled member's factory never runs");

    let enabled =
        build(reference().with(renderer_probe("probe-render-on", &on))).expect("the probe admits");
    let ui = built::<UiSlot>(&enabled)
        .expect("the ui slot builds")
        .expect("the ui slot is built");
    assert!(
        ui.supports(PROBE_TYPE),
        "negative control: the enabled renderer is built"
    );
    assert_eq!(on.get(), 1);
}

#[test]
fn a_disabled_members_recorder_is_never_built() {
    let on = Arc::new(Counter::default());
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(recorder_probe("probe-rec-on", &on))
        .with(recorder_probe("probe-rec-off", &off))
        .declare()
        .expect("both probes declare")
        .admit(&profile(&["probe-rec-off"]))
        .expect("admission succeeds");
    let recorders = built::<AccessRecorderSlot>(&admitted)
        .expect("the recorder slot builds")
        .expect("the recorder slot is built");
    assert_eq!(recorders.to_vec().len(), 1);
    assert_eq!(off.get(), 0, "a disabled member's recorder never runs");
    assert_eq!(on.get(), 1, "the enabled member's recorder runs");
}

#[test]
fn two_toolsets_with_one_id_are_refused() {
    let admitted = build(
        reference()
            .with(toolset_probe("probe-toolset-a", "probe_toolset"))
            .with(toolset_probe("probe-toolset-b", "probe_toolset")),
    )
    .expect("both probes admit");
    let seed = Dependencies::default().with(systemprompt_test_fixtures::fixture_services_view());
    let error = built_with::<ToolsetSlot>(&admitted, seed).expect_err("a duplicate id refuses");
    assert!(
        matches!(&error, SlotError::DuplicateId { slot: "mcp_toolsets", id } if id == "probe_toolset"),
        "{error:?}"
    );
}

#[test]
fn a_hosted_toolset_no_member_contributes_refuses_the_build() {
    let admitted = build(reference().with(toolset_probe("probe-toolset", "probe_toolset")))
        .expect("the probe admits");
    let error = built_with::<ToolsetSlot>(
        &admitted,
        Dependencies::default().with(services_hosting("ghost_toolset")),
    )
    .expect_err("an unregistered hosted toolset refuses");
    let SlotError::Refused { slot, source } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(*slot, "mcp_toolsets");
    let refused = source
        .downcast_ref::<UnregisteredToolsets>()
        .expect("the refusal names the unregistered toolsets");
    assert_eq!(refused.missing.len(), 1);
    assert_eq!(refused.missing[0].toolset, "ghost_toolset");

    let toolsets: Arc<Toolsets> = built_with::<ToolsetSlot>(
        &admitted,
        Dependencies::default().with(services_hosting("probe_toolset")),
    )
    .expect("negative control: a contributed hosted toolset builds")
    .expect("the toolset slot is built");
    assert!(toolsets.find("probe_toolset").is_some());
}

#[test]
fn a_disabled_members_toolset_is_not_hosted() {
    let admitted = reference()
        .with(toolset_probe("probe-toolset-off", "probe_toolset"))
        .declare()
        .expect("the probe declares")
        .admit(&profile(&["probe-toolset-off"]))
        .expect("admission succeeds");
    let error = built_with::<ToolsetSlot>(
        &admitted,
        Dependencies::default().with(services_hosting("probe_toolset")),
    )
    .expect_err("a disabled member's toolset is not contributed");
    assert!(
        matches!(
            error,
            SlotError::Refused {
                slot: "mcp_toolsets",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn agent_runs_api_holds_one_route_set_in_the_agent_group() {
    let admitted = build(reference()).expect("the reference composition admits");
    let sets: Vec<_> = admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == "agent_runs_api")
        .collect();
    assert_eq!(sets.len(), 1, "{sets:?}");
    assert_eq!(
        sets[0].set.base(),
        BaseRef::Path(BasePath::Api(ApiBase::AgentRuns))
    );
    assert_eq!(
        sets[0].set.group(),
        RouteGroup::Agent,
        "served on every role that serves the agent protocol, workers included"
    );
    assert_eq!(sets[0].set.defs().len(), 7);
}

#[test]
fn admitted_run_origins_are_those_of_enabled_members() {
    let admitted = reference()
        .with(origin_probe("probe-origin-on", "probe_on"))
        .with(origin_probe("probe-origin-off", "probe_off"))
        .declare()
        .expect("both probes declare")
        .admit(&profile(&["probe-origin-off"]))
        .expect("admission succeeds");
    let origins = AdmittedRunOrigins::of(admitted.built());
    assert!(origins.contains("probe_on"));
    assert!(!origins.contains("probe_off"));
}
