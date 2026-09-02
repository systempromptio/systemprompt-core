use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema,
    HostKind, ProbeEnv, ProfileGenInputs, ProfileRemoval, ProfileState, effective_surfaces,
    has_surface_override,
};
use systemprompt_bridge::proxy::LoopbackEndpoint;
use systemprompt_models::profile::ApiSurface;

struct BareHost;

static BARE_SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: &["alpha"],
    display_keys: &["alpha", "beta"],
};

impl HostApp for BareHost {
    fn id(&self) -> &'static str {
        "bare-host"
    }

    fn display_name(&self) -> &'static str {
        "Bare Host"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &BARE_SCHEMA
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            host_running: false,
            host_processes: Vec::new(),
            app_installed: AppInstallState::Unknown,
            probed_at_unix: u64::from(env.proxy_port),
        }
    }

    fn generate_profile(&self, _inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "bare host generates nothing",
        ))
    }

    fn install_profile(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
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
    assert_eq!(host.kind(), HostKind::DesktopApp);
    assert_eq!(host.description(), "");
    assert_eq!(
        host.icon_id(),
        "bare-host",
        "the icon defaults to the host id"
    );
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
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
    assert!(
        err.to_string().contains("open not implemented"),
        "got {err}"
    );
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
fn probe_env_carries_the_port_and_secret_fingerprint_of_the_endpoint_it_was_built_from() {
    let endpoint = LoopbackEndpoint::new(51999, None);
    let env = ProbeEnv::new(&endpoint, std::sync::Arc::default());
    assert_eq!(env.proxy_port, 51999);
    assert_eq!(
        env.loopback_secret_fingerprint,
        endpoint.secret_fingerprint(),
        "the probe env fingerprint is the endpoint's, not a fresh read"
    );
    let snapshot = BareHost.probe(&env);
    assert_eq!(
        snapshot.probed_at_unix, 51999,
        "the probe was handed the injected port, not process state"
    );
    assert_eq!(snapshot.app_installed, AppInstallState::Unknown);
}

#[test]
fn a_host_without_an_override_keeps_its_declared_surfaces() {
    let overrides = surface_overrides(&[("other-host", &["openai"])]);
    assert_eq!(
        effective_surfaces("codex-cli", &[ApiSurface::OpenAi], &overrides),
        vec![ApiSurface::OpenAi]
    );
    assert!(!has_surface_override("codex-cli", &overrides));
    assert!(has_surface_override("other-host", &overrides));
}

#[test]
fn an_override_replaces_the_declared_surfaces_entirely() {
    let overrides = surface_overrides(&[("codex-cli", &["anthropic", "gemini"])]);
    assert_eq!(
        effective_surfaces("codex-cli", &[ApiSurface::OpenAi], &overrides),
        vec![ApiSurface::Anthropic, ApiSurface::Gemini],
        "the override wins outright rather than merging with the default"
    );
}

#[test]
fn an_unknown_surface_tag_in_an_override_is_dropped_not_defaulted() {
    let overrides = surface_overrides(&[("codex-cli", &["not-a-surface", "openai"])]);
    assert_eq!(
        effective_surfaces("codex-cli", &[ApiSurface::Anthropic], &overrides),
        vec![ApiSurface::OpenAi],
        "an unreadable tag must not fall back to the host default"
    );
}

#[test]
fn an_override_of_only_unknown_tags_yields_no_surfaces_at_all() {
    let overrides = surface_overrides(&[("codex-cli", &["nonsense"])]);
    assert!(
        effective_surfaces("codex-cli", &[ApiSurface::Anthropic], &overrides).is_empty(),
        "an override that names nothing recognisable narrows to nothing"
    );
}
