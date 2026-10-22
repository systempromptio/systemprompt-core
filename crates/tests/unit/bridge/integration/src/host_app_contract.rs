use std::collections::BTreeMap;

use systemprompt_bridge::drift::{DesiredProfile, EntryRole};
use systemprompt_bridge::gateway::model_view::{effective_surfaces, has_surface_override};
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, DesiredError, GeneratedProfile, HostApp, HostAppError,
    HostAppKind, HostAppSnapshot, ProbeEnv, ProfileFactsError, ProfileGenInputs, ProfileInstalled,
    ProfileRemoval, ProfileState,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::ApiSurface;

use crate::support::{env_with, facts, unreadable_env};

struct BareHost;

impl HostApp for BareHost {
    fn id(&self) -> HostKind {
        HostKind::Hermes
    }

    fn display_name(&self) -> &'static str {
        "Bare Host"
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            probe_error: None,
            host_running: Some(false),
            host_processes: Vec::new(),
            app_installed: AppInstallState::Unknown,
            probed_at_unix: u64::from(env.proxy_port),
            update_needs_approval: false,
        }
    }

    fn desired(&self, inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        let mut desired = DesiredProfile::new();
        desired.push(
            "endpoint",
            inputs.gateway_base_url.clone(),
            EntryRole::Endpoint,
        );
        Ok(desired)
    }

    fn app_installed(
        &self,
        _start_menu: &systemprompt_bridge::probe_cache::StartMenuCache,
    ) -> AppInstallState {
        AppInstallState::Unknown
    }

    fn generate_profile(
        &self,
        _inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "bare host generates nothing",
        )
        .into())
    }

    fn install_profile(&self, _path: &str) -> Result<ProfileInstalled, HostAppError> {
        Ok(ProfileInstalled::ok())
    }

    fn install_action_label(&self) -> &'static str {
        "install bare"
    }
}

fn surface_overrides(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(host, tags)| {
            (
                (*host).to_owned(),
                tags.iter().map(|t| (*t).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn a_host_that_implements_only_the_required_methods_gets_the_default_contract() {
    let host = BareHost;
    assert!(
        host.can_open(),
        "a host is openable unless it says otherwise"
    );
    assert_eq!(
        host.kind(),
        HostAppKind::CliTool,
        "the packaging is the host id's, never restated by the host"
    );
    assert_eq!(host.profile_identity(), host.id());
    assert_eq!(host.description(), "");
    assert_eq!(host.icon_id(), "hermes", "the icon defaults to the host id");
    assert_eq!(host.config_format(), ConfigFormat::Json);
    assert_eq!(host.download_url(), "");
    assert!(
        host.accepted_surfaces().is_empty(),
        "no declared surfaces means the host accepts whatever the gateway offers"
    );
}

#[test]
fn the_default_open_is_an_unsupported_error_rather_than_a_silent_success() {
    let err = BareHost
        .open()
        .expect_err("a host with no window must not report a successful open");
    assert!(
        matches!(err, HostAppError::OpenUnsupported),
        "the default open is a typed unsupported error, got {err:?}"
    );
    assert!(!err.is_refusal(), "an unsupported open is not a refusal");
}

#[test]
fn the_default_removal_asks_the_user_rather_than_claiming_a_removal() {
    let removal = BareHost
        .remove_profile()
        .expect("the default removal never fails");
    match removal {
        ProfileRemoval::ManualStepRequired { instruction } => {
            assert!(
                instruction.contains("by hand"),
                "the instruction names the manual step, got {instruction}"
            );
        },
        other => panic!("expected ManualStepRequired, got {other:?}"),
    }
}

#[test]
fn the_probe_is_handed_the_injected_port_not_process_state() {
    let mut env: ProbeEnv = env_with(facts());
    env.proxy_port = 51999;
    let snapshot = BareHost.probe(&env);
    assert_eq!(snapshot.probed_at_unix, 51999);
    assert_eq!(snapshot.app_installed, AppInstallState::Unknown);
}

#[test]
fn a_hosts_desired_settings_are_rendered_from_the_shared_facts() {
    let desired = env_with(facts())
        .desired_for(&BareHost)
        .expect("facts are readable");
    assert_eq!(desired.entries().len(), 1);
    assert_eq!(desired.entries()[0].value, facts().gateway_base_url);
}

#[test]
fn unreadable_facts_make_every_hosts_desired_settings_an_error() {
    let env = unreadable_env(ProfileFactsError::LoopbackSecret(std::io::Error::other(
        "no secret",
    )));
    assert!(matches!(
        env.desired_for(&BareHost),
        Err(DesiredError::Facts(_))
    ));
}

#[test]
fn by_default_no_sync_failure_is_observed_by_the_probe() {
    assert!(!BareHost.probe_observes("hermes", true));
    assert!(!BareHost.probe_observes("hermes", false));
}

#[test]
fn a_host_without_an_override_keeps_its_declared_surfaces() {
    let overrides = surface_overrides(&[("hermes", &["openai"])]);
    assert_eq!(
        effective_surfaces(HostKind::CodexCli, &[ApiSurface::OpenAi], &overrides),
        vec![ApiSurface::OpenAi]
    );
    assert!(!has_surface_override(HostKind::CodexCli, &overrides));
    assert!(has_surface_override(HostKind::Hermes, &overrides));
}

#[test]
fn an_override_replaces_the_declared_surfaces_entirely() {
    let overrides = surface_overrides(&[("codex-cli", &["anthropic", "gemini"])]);
    assert_eq!(
        effective_surfaces(HostKind::CodexCli, &[ApiSurface::OpenAi], &overrides),
        vec![ApiSurface::Anthropic, ApiSurface::Gemini],
        "the override wins outright rather than merging with the default"
    );
}

#[test]
fn an_unknown_surface_tag_in_an_override_is_dropped_not_defaulted() {
    let overrides = surface_overrides(&[("codex-cli", &["not-a-surface", "openai"])]);
    assert_eq!(
        effective_surfaces(HostKind::CodexCli, &[ApiSurface::Anthropic], &overrides),
        vec![ApiSurface::OpenAi],
        "an unreadable tag must not fall back to the host default"
    );
}

#[test]
fn an_override_of_only_unknown_tags_yields_no_surfaces_at_all() {
    let overrides = surface_overrides(&[("codex-cli", &["nonsense"])]);
    assert!(
        effective_surfaces(HostKind::CodexCli, &[ApiSurface::Anthropic], &overrides).is_empty(),
        "an override that names nothing recognisable narrows to nothing"
    );
}

#[test]
fn claude_desktop_is_the_only_narrowing_host() {
    for host in systemprompt_bridge::integration::host_apps() {
        if host.accepted_surfaces().is_empty() {
            continue;
        }
        assert_eq!(
            host.id(),
            HostKind::ClaudeDesktop,
            "every host is offered the whole advertised catalog; Claude Desktop is the \
             single carve-out because it rejects non-Claude ids in inferenceModels"
        );
        assert_eq!(host.accepted_surfaces(), &[ApiSurface::Anthropic]);
    }
}
