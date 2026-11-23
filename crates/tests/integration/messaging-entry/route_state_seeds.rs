//! No route state of any admitted member reads a host-sealed seed. The walk
//! covers the reference composition built with Slack and Teams, so every
//! core member is in it: each admitted route set and gateway route resolves
//! its state through the resolution the mount builds, which refuses every
//! host-sealed seed whatever the owner's standing, so a route state that
//! still read one fails here naming the set and the type. The route sets
//! moved off host seeds (content, marketplace, agent runs, Slack, Teams) are
//! named, so dropping one from the walk fails too.

use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_extension::StandingTag;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_test_fixtures::{ensure_messaging_bootstrap, fixture_app_context_on_bootstrap};
use systemprompt_traits::{CustomSecrets, SecretStore};

const MOVED: [&str; 5] = [
    "content-http",
    "marketplace-http",
    "agent_runs_api",
    "slack",
    "teams",
];

#[tokio::test]
async fn no_route_state_of_any_admitted_member_resolves_a_host_sealed_seed() -> anyhow::Result<()> {
    let bootstrap = ensure_messaging_bootstrap();
    let ctx = fixture_app_context_on_bootstrap(
        bootstrap,
        Config::clone(bootstrap.boot_config()),
        crate::common::messaging_admitted(),
    )
    .await?;
    let kernel = ctx.kernel();
    let admitted = ctx.admitted();

    let mut owners = BTreeSet::new();
    let mut refused = Vec::new();
    for set in admitted.routes() {
        owners.insert(set.owner.as_str().to_owned());
        let resolved = kernel.route_scoped(&set.owner, set.standing, &set.handles);
        assert!(
            resolved.optional::<Arc<Config>>().is_none(),
            "{}",
            set.owner
        );
        assert!(
            resolved.optional::<Arc<dyn SecretStore>>().is_none(),
            "{}",
            set.owner
        );
        assert!(
            resolved.optional::<CustomSecrets>().is_none(),
            "{}",
            set.owner
        );
        assert!(
            resolved.optional::<SystemAdmin>().is_none(),
            "{}",
            set.owner
        );
        if let Err(missing) = set.set.materialize(&resolved) {
            refused.push(format!("{}: {}", set.owner, missing.type_name()));
        }
    }
    for route in admitted.gateway_routes() {
        let standing = admitted
            .built()
            .standing_of(&route.owner)
            .unwrap_or(StandingTag::Install);
        if let Err(missing) =
            route
                .decl
                .router
                .materialize(&kernel.route_scoped(&route.owner, standing, &[]))
        {
            refused.push(format!(
                "{} (gateway): {}",
                route.owner,
                missing.type_name()
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "route states that need a withheld value: {refused:?}"
    );
    for owner in MOVED {
        assert!(
            owners.contains(owner),
            "{owner} is admitted with its route sets: {owners:?}"
        );
    }
    Ok(())
}
