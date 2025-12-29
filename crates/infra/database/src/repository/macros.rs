#[macro_export]
macro_rules! impl_repository_new {
    ($repo:ty) => {
        impl $repo {
            #[must_use]
            pub fn new(db: &$crate::DbPool) -> Self {
                let pool = db.get_postgres_pool().expect("Database must be PostgreSQL");
                Self { pool }
            }

            pub fn try_new(db: &$crate::DbPool) -> Result<Self, $crate::RepositoryError> {
                let pool = db.get_postgres_pool().ok_or_else(|| {
                    $crate::RepositoryError::internal("Database must be PostgreSQL")
                })?;
                Ok(Self { pool })
            }

            #[must_use]
            pub const fn from_pool(pool: $crate::PgDbPool) -> Self {
                Self { pool }
            }
        }
    };
}

#[macro_export]
macro_rules! define_repository {
    ($repo:ident) => {
        #[derive(Debug, Clone)]
        pub struct $repo {
            pool: $crate::PgDbPool,
        }

        $crate::impl_repository_new!($repo);
    };

    ($repo:ident, $visibility:vis) => {
        #[derive(Debug, Clone)]
        $visibility struct $repo {
            pool: $crate::PgDbPool,
        }

        $crate::impl_repository_new!($repo);
    };
}

#[macro_export]
macro_rules! impl_repository_pool {
    ($repo:ty) => {
        impl $repo {
            #[must_use]
            pub fn pool(&self) -> &$crate::PgDbPool {
                &self.pool
            }

            #[must_use]
            pub fn pg_pool(&self) -> &::sqlx::PgPool {
                &self.pool
            }
        }
    };
}
