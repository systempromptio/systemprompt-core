//! [`ConnectionBudget`]: the division of a node's Postgres connections.
//!
//! The budget is split between the main pools, the session pool and the
//! hosted children the node spawns; [`BudgetError`] says the pools it must
//! open do not fit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A node's declared Postgres connection budget (`database.connection_budget`)
/// and the rule that divides it.
///
/// For a node: a fifth of the budget, in whole [`CHILD_POOL_CONNECTIONS`]
/// shares, is reserved for the hosted children it spawns; a separate session
/// pool takes a tenth (at least [`MIN_SESSION_CONNECTIONS`]: the
/// notification hub's `LISTEN`, the instance claim and one lock or
/// migration session); the rest is the main pool, split evenly between the
/// read and write pools when `database_write_url` names a second one. An
/// explicit `database.pool.max_connections` or `session_max_connections`
/// is kept and checked against the budget instead. A hosted child takes one
/// fixed share and opens no hub session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionBudget {
    total: u32,
}

pub const CHILD_POOL_CONNECTIONS: u32 = 2;
pub const MIN_SESSION_CONNECTIONS: u32 = 3;
const MIN_MAIN_POOL_CONNECTIONS: u32 = 2;
const CHILD_RESERVE_DIVISOR: u32 = 5;
const SESSION_DIVISOR: u32 = 10;

/// Whose pools a budget is divided for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSeat {
    Node,
    Child,
}

/// Which separate pools the node's secrets open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolShape {
    pub separate_write: bool,
    pub separate_session: bool,
}

/// Pool sizes an operator pinned in `database.pool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinnedPools {
    pub main: Option<u32>,
    pub session: Option<u32>,
}

/// How a budget was divided.
///
/// `main` sizes each of the `main_pools` (the primary pool, and the write
/// pool when one is separate), `session` the separate session pool, and
/// `children` is the share reserved for hosted children, not opened by the
/// node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSizes {
    pub main: u32,
    pub main_pools: u32,
    pub session: u32,
    pub children: u32,
}

impl PoolSizes {
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.main * self.main_pools + self.session + self.children
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BudgetError {
    #[error(
        "database.connection_budget {budget} is below the {required} connections this node's \
         pools, session pool and children need"
    )]
    TooSmall { budget: u32, required: u32 },
    #[error(
        "database.pool sizes need {parts} connections, more than database.connection_budget {budget}"
    )]
    PartsExceed { budget: u32, parts: u32 },
}

impl ConnectionBudget {
    #[must_use]
    pub const fn new(total: u32) -> Self {
        Self { total }
    }

    #[must_use]
    pub const fn total(self) -> u32 {
        self.total
    }

    pub fn divide(
        self,
        seat: PoolSeat,
        shape: PoolShape,
        pinned: PinnedPools,
    ) -> Result<PoolSizes, BudgetError> {
        let main_pools = if shape.separate_write { 2 } else { 1 };
        let sizes = match seat {
            PoolSeat::Child => PoolSizes {
                main: CHILD_POOL_CONNECTIONS / main_pools,
                main_pools,
                session: u32::from(shape.separate_session),
                children: 0,
            },
            PoolSeat::Node => self.divide_node(shape, pinned, main_pools)?,
        };
        if sizes.total() > self.total {
            return Err(BudgetError::PartsExceed {
                budget: self.total,
                parts: sizes.total(),
            });
        }
        Ok(sizes)
    }

    fn divide_node(
        self,
        shape: PoolShape,
        pinned: PinnedPools,
        main_pools: u32,
    ) -> Result<PoolSizes, BudgetError> {
        let children =
            self.total / CHILD_RESERVE_DIVISOR / CHILD_POOL_CONNECTIONS * CHILD_POOL_CONNECTIONS;
        let session = if shape.separate_session {
            pinned
                .session
                .unwrap_or_else(|| (self.total / SESSION_DIVISOR).max(MIN_SESSION_CONNECTIONS))
        } else {
            0
        };
        let required = children
            + if shape.separate_session {
                MIN_SESSION_CONNECTIONS
            } else {
                0
            }
            + MIN_MAIN_POOL_CONNECTIONS * main_pools;
        if self.total < required {
            return Err(BudgetError::TooSmall {
                budget: self.total,
                required,
            });
        }
        let main = pinned
            .main
            .unwrap_or_else(|| self.total.saturating_sub(children + session) / main_pools);
        if main < MIN_MAIN_POOL_CONNECTIONS {
            return Err(BudgetError::TooSmall {
                budget: self.total,
                required,
            });
        }
        Ok(PoolSizes {
            main,
            main_pools,
            session,
            children,
        })
    }
}
