use systemprompt_database::{
    BudgetError, CHILD_POOL_CONNECTIONS, ConnectionBudget, MIN_SESSION_CONNECTIONS, PinnedPools,
    PoolSeat, PoolShape, PoolSizes,
};

const BOTH: PoolShape = PoolShape {
    separate_write: true,
    separate_session: true,
};

#[test]
fn a_node_on_one_pool_reserves_the_children_share_and_keeps_the_rest() {
    let sizes = ConnectionBudget::new(20)
        .divide(PoolSeat::Node, PoolShape::default(), PinnedPools::default())
        .expect("divides");
    assert_eq!(
        sizes,
        PoolSizes {
            main: 16,
            main_pools: 1,
            session: 0,
            children: 4,
        }
    );
    assert!(sizes.total() <= 20);
}

#[test]
fn a_node_with_write_and_session_pools_splits_the_main_share_evenly() {
    let sizes = ConnectionBudget::new(20)
        .divide(PoolSeat::Node, BOTH, PinnedPools::default())
        .expect("divides");
    assert_eq!(
        sizes,
        PoolSizes {
            main: 6,
            main_pools: 2,
            session: MIN_SESSION_CONNECTIONS,
            children: 4,
        }
    );
    assert_eq!(sizes.total(), 19);
}

#[test]
fn a_budget_smaller_than_the_minimum_parts_refuses_to_boot() {
    let refused = ConnectionBudget::new(4).divide(PoolSeat::Node, BOTH, PinnedPools::default());
    assert_eq!(
        refused,
        Err(BudgetError::TooSmall {
            budget: 4,
            required: MIN_SESSION_CONNECTIONS + 4,
        })
    );
}

#[test]
fn pinned_pools_larger_than_the_budget_refuse_to_boot() {
    let refused = ConnectionBudget::new(20).divide(
        PoolSeat::Node,
        BOTH,
        PinnedPools {
            main: Some(50),
            session: None,
        },
    );
    assert_eq!(
        refused,
        Err(BudgetError::PartsExceed {
            budget: 20,
            parts: 50 * 2 + MIN_SESSION_CONNECTIONS + 4,
        })
    );
}

#[test]
fn a_pinned_session_pool_is_kept_and_counted() {
    let sizes = ConnectionBudget::new(40)
        .divide(
            PoolSeat::Node,
            BOTH,
            PinnedPools {
                main: None,
                session: Some(6),
            },
        )
        .expect("divides");
    assert_eq!(sizes.session, 6);
    assert_eq!(sizes.main, (40 - 8 - 6) / 2);
    assert!(sizes.total() <= 40);
}

#[test]
fn a_hosted_child_takes_one_fixed_share_whatever_the_budget() {
    for budget in [10, 100, 1000] {
        let sizes = ConnectionBudget::new(budget)
            .divide(PoolSeat::Child, BOTH, PinnedPools::default())
            .expect("divides");
        assert_eq!(sizes.main * sizes.main_pools, CHILD_POOL_CONNECTIONS);
        assert_eq!(sizes.session, 1);
        assert_eq!(sizes.children, 0);
    }
}
