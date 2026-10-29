use std::path::PathBuf;

use systemprompt_bridge::config::gateways::{self, DeliveredInstance, GatewayProfileId};
use systemprompt_bridge::config::paths;
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::integration::lifecycle::{self, LifecycleError};
use systemprompt_identifiers::ValidatedUrl;
use tempfile::TempDir;

const GATEWAY_A: &str = "https://acme.example.com";
const GATEWAY_B: &str = "https://client-b.example.com";

fn sandbox(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/config"))),
        ("XDG_STATE_HOME", Some(format!("{root}/state"))),
        ("XDG_DATA_HOME", Some(format!("{root}/data"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/cache"))),
        ("CLAUDE_CONFIG_DIR", Some(format!("{root}/claude"))),
        (
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(format!("{root}/org-plugins")),
        ),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
    ]
}

fn configure_default(gateway: &str) {
    let profile = gateways::active().unwrap();
    std::fs::create_dir_all(&profile.dir).unwrap();
    std::fs::write(
        &profile.config_file,
        format!("gateway_url = \"{gateway}\"\n"),
    )
    .unwrap();
}

struct Seeded {
    owned_plugin: PathBuf,
    foreign_plugin: PathBuf,
    metadata: PathBuf,
}

fn seed_synced_state(gateway: &str) -> Seeded {
    let metadata = paths::bridge_metadata_dir().unwrap();
    std::fs::create_dir_all(&metadata).unwrap();
    std::fs::write(
        metadata.join(paths::LAST_SYNC_SENTINEL),
        serde_json::json!({
            "gateway": gateway,
            "present_plugins": ["acme-plugin"],
            "enabled_hosts": ["codex-cli"],
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(metadata.join(paths::MCP_SERVERS_FRAGMENT), "{}").unwrap();
    std::fs::write(metadata.join(paths::USER_FRAGMENT), "{}").unwrap();
    let root = paths::org_plugins_effective().unwrap().path;
    let owned_plugin = root.join("acme-plugin");
    let foreign_plugin = root.join("someone-elses-plugin");
    std::fs::create_dir_all(&owned_plugin).unwrap();
    std::fs::create_dir_all(&foreign_plugin).unwrap();
    Seeded {
        owned_plugin,
        foreign_plugin,
        metadata,
    }
}

fn deliver(allow_user_disable: bool) {
    let profile = gateways::active().unwrap();
    gateways::write_delivered(
        &profile,
        &DeliveredInstance {
            gateway: Some(ValidatedUrl::try_new(GATEWAY_A).unwrap()),
            window: None,
            branding: None,
            allow_user_disable,
        },
    )
    .unwrap();
}

#[test]
fn switching_gateways_clears_what_the_outgoing_one_delivered() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        configure_default(GATEWAY_A);
        let seeded = seed_synced_state(GATEWAY_A);
        let next = gateways::add(GATEWAY_B).unwrap();
        let ctx = BridgeContext::start(ProxyMode::Attach, std::sync::Arc::new(|| None))
            .expect("runtime builds");

        let report = lifecycle::switch(&ctx, &next).expect("switch succeeds");

        assert_eq!(report.profile.id, next);
        assert_eq!(gateways::active().unwrap().id, next);
        assert!(
            !seeded.owned_plugin.exists(),
            "the outgoing gateway's plugin is removed"
        );
        assert!(
            seeded.foreign_plugin.exists(),
            "a plugin the bridge did not place stays"
        );
        assert_eq!(report.released.foreign_plugins, ["someone-elses-plugin"]);
        for fragment in [
            paths::LAST_SYNC_SENTINEL,
            paths::MCP_SERVERS_FRAGMENT,
            paths::USER_FRAGMENT,
        ] {
            assert!(
                !seeded.metadata.join(fragment).exists(),
                "{fragment} survived the switch"
            );
        }
        assert_eq!(
            ctx.proxy
                .runtime_config()
                .load()
                .gateway_base
                .as_str()
                .trim_end_matches('/'),
            GATEWAY_B,
            "the proxy routes to the new gateway without a restart"
        );
        assert!(matches!(
            lifecycle::switch(&ctx, &next),
            Err(LifecycleError::AlreadyActive(_))
        ));
        assert!(matches!(
            lifecycle::switch(&ctx, &GatewayProfileId::try_new("nope").unwrap()),
            Err(LifecycleError::Profile(_))
        ));
    });
}

#[test]
fn disable_is_refused_until_the_gateway_allows_it_and_touches_nothing_when_refused() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        configure_default(GATEWAY_A);
        let seeded = seed_synced_state(GATEWAY_A);
        let ctx = BridgeContext::start(ProxyMode::Attach, std::sync::Arc::new(|| None))
            .expect("runtime builds");

        assert!(matches!(
            lifecycle::disable(&ctx),
            Err(LifecycleError::NoDeliveredPolicy)
        ));
        deliver(false);
        assert!(matches!(
            lifecycle::disable(&ctx),
            Err(LifecycleError::DisableNotAllowed)
        ));
        assert!(!systemprompt_bridge::config::load().unwrap().is_disabled());
        assert!(
            seeded.owned_plugin.exists(),
            "a refused disable removes nothing"
        );
        assert!(seeded.metadata.join(paths::LAST_SYNC_SENTINEL).exists());
    });
}

#[test]
fn an_allowed_disable_releases_the_hosts_and_stops_routing_until_enabled() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        configure_default(GATEWAY_A);
        let seeded = seed_synced_state(GATEWAY_A);
        deliver(true);
        let ctx = BridgeContext::start(ProxyMode::Attach, std::sync::Arc::new(|| None))
            .expect("runtime builds");

        let report = lifecycle::disable(&ctx).expect("the gateway allows it");
        assert!(!report.already_disabled);
        assert!(systemprompt_bridge::config::load().unwrap().is_disabled());
        assert!(ctx.proxy.runtime_config().load().disabled);
        assert!(!seeded.owned_plugin.exists());
        assert!(!seeded.metadata.join(paths::LAST_SYNC_SENTINEL).exists());

        assert!(lifecycle::disable(&ctx).unwrap().already_disabled);
        assert!(lifecycle::enable(&ctx).unwrap());
        assert!(!systemprompt_bridge::config::load().unwrap().is_disabled());
        assert!(!ctx.proxy.runtime_config().load().disabled);
        assert!(!lifecycle::enable(&ctx).unwrap(), "enable is idempotent");
    });
}
