use std::collections::BTreeMap;

use systemprompt_bridge::drift::{DesiredProfile, EntryRole};
use systemprompt_bridge::gateway::model_view::{HostClients, host_client};
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, DesiredError, GeneratedProfile, HostApp, HostAppError,
    HostAppKind, HostAppSnapshot, ProbeEnv, ProfileFactsError, ProfileGenInputs, ProfileInstalled,
    ProfileRemoval, ProfileState,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::ClientFamily;

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
        "a desktop host is openable unless it says otherwise"
    );
    assert_eq!(host.kind(), HostAppKind::DesktopApp);
    assert_eq!(host.description(), "");
    assert_eq!(host.icon_id(), "hermes", "the icon defaults to the host id");
    assert_eq!(host.config_format(), ConfigFormat::Json);
    assert_eq!(host.download_url(), "");
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
fn a_host_without_a_filter_keeps_its_own_family() {
    let filters = surface_overrides(&[("hermes", &["claude"])]);
    let codex = HostClients::for_host(HostKind::CodexCli, &filters);
    assert_eq!(codex.families, vec![ClientFamily::OpenAi]);
    assert!(!codex.filtered);
    assert!(HostClients::for_host(HostKind::Hermes, &filters).filtered);
}

#[test]
fn a_filter_replaces_the_host_family_entirely() {
    let filters = surface_overrides(&[("codex-cli", &["claude", "gemini"])]);
    assert_eq!(
        HostClients::for_host(HostKind::CodexCli, &filters).families,
        vec![ClientFamily::Claude, ClientFamily::Gemini],
        "the filter wins outright rather than merging with the default"
    );
}

#[test]
fn an_unknown_tag_in_a_filter_is_dropped_not_defaulted() {
    let filters = surface_overrides(&[("codex-cli", &["anthropic", "openai"])]);
    assert_eq!(
        HostClients::for_host(HostKind::CodexCli, &filters).families,
        vec![ClientFamily::OpenAi],
        "a pre-0.73 surface tag is not a client family and must not fall back"
    );
}

#[test]
fn a_filter_of_only_unknown_tags_yields_no_families_at_all() {
    let filters = surface_overrides(&[("codex-cli", &["nonsense"])]);
    assert!(
        HostClients::for_host(HostKind::CodexCli, &filters)
            .families
            .is_empty(),
        "a filter that names nothing recognisable narrows to nothing"
    );
}

#[test]
fn every_registered_host_has_a_client_family() {
    for host in systemprompt_bridge::integration::host_apps() {
        let expected = match host.id() {
            HostKind::ClaudeCode | HostKind::ClaudeDesktop => ClientFamily::Claude,
            HostKind::CodexCli | HostKind::OpenCode | HostKind::Hermes => ClientFamily::OpenAi,
        };
        assert_eq!(host_client(host.id()), expected, "{:?}", host.id());
    }
}
