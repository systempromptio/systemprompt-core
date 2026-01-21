use systemprompt_database::DbPool;

#[derive(Clone)]
pub struct McpState {
    db_pool: DbPool,
}

impl std::fmt::Debug for McpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpState")
            .field("db_pool", &"DbPool")
            .finish()
    }
}

impl McpState {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }
}
