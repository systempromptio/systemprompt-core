use systemprompt_manifest::profile::{
    GovernanceConfig, NodeRole, PoolConfig, QuotaConfig, RoleMaxConnections,
};

use crate::profile_validation::{errors_of, valid_profile};

fn pool_with_roles(by_role: RoleMaxConnections) -> PoolConfig {
    PoolConfig {
        max_connections: Some(40),
        max_connections_by_role: Some(by_role),
        ..PoolConfig::default()
    }
}

#[test]
fn a_role_override_wins_and_other_roles_fall_back() {
    let pool = pool_with_roles(RoleMaxConnections {
        gateway: Some(15),
        ..RoleMaxConnections::default()
    });
    assert_eq!(pool.max_connections_for(NodeRole::Gateway), Some(15));
    assert_eq!(pool.max_connections_for(NodeRole::Api), Some(40));
    assert_eq!(pool.max_connections_for(NodeRole::Worker), Some(40));
    assert_eq!(pool.max_connections_for(NodeRole::All), Some(40));
}

#[test]
fn no_pool_ceiling_leaves_the_engine_default() {
    assert_eq!(
        PoolConfig::default().max_connections_for(NodeRole::Gateway),
        None
    );
}

#[test]
fn role_overrides_parse_from_yaml() {
    let pool: PoolConfig = serde_yaml::from_str(
        "max_connections: 30\nmax_connections_by_role:\n  gateway: 12\n  worker: 30\n",
    )
    .expect("pool yaml");
    assert_eq!(pool.max_connections_for(NodeRole::Gateway), Some(12));
    assert_eq!(pool.max_connections_for(NodeRole::Runner), Some(30));
    assert!(
        serde_yaml::from_str::<PoolConfig>("max_connections_by_role:\n  admin: 3\n").is_err(),
        "the retired admin role is rejected"
    );
}

#[test]
fn an_out_of_range_role_ceiling_is_rejected() {
    for bad in [0, 501] {
        let mut p = valid_profile();
        p.database.pool = Some(pool_with_roles(RoleMaxConnections {
            worker: Some(bad),
            ..RoleMaxConnections::default()
        }));
        assert!(errors_of(&p).contains("database.pool.max_connections_by_role.worker"));
    }
}

#[test]
fn in_range_role_ceilings_are_accepted() {
    let mut p = valid_profile();
    p.database.pool = Some(pool_with_roles(RoleMaxConnections {
        all: Some(50),
        gateway: Some(1),
        api: Some(20),
        worker: Some(500),
        runner: Some(10),
    }));
    assert!(!errors_of(&p).contains("database.pool"));
}

fn with_quota(quota: QuotaConfig) -> String {
    let mut p = valid_profile();
    p.governance = Some(GovernanceConfig {
        quota,
        ..GovernanceConfig::default()
    });
    errors_of(&p)
}

#[test]
fn quota_shards_default_to_eight_shared_and_one_principal() {
    let p = valid_profile();
    let quota = p.quota_config();
    assert_eq!(quota.shared_subject_shards, 8);
    assert_eq!(quota.principal_subject_shards, 1);
    assert!(!errors_of(&p).contains("governance.quota"));
}

#[test]
fn quota_shard_counts_outside_one_to_sixty_four_are_rejected() {
    for bad in [0, 65] {
        assert!(
            with_quota(QuotaConfig {
                shared_subject_shards: bad,
                ..QuotaConfig::default()
            })
            .contains("governance.quota.shared_subject_shards")
        );
        assert!(
            with_quota(QuotaConfig {
                principal_subject_shards: bad,
                ..QuotaConfig::default()
            })
            .contains("governance.quota.principal_subject_shards")
        );
    }
    assert!(
        !with_quota(QuotaConfig {
            shared_subject_shards: 64,
            principal_subject_shards: 1,
        })
        .contains("governance.quota")
    );
}
