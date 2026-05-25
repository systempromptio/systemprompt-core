//! Collapsing a contiguous range of applied migrations into a single
//! version-0 baseline row.

use super::MigrationService;
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::info;

#[derive(Debug, Clone)]
pub struct SquashPlan {
    pub extension_id: String,
    pub through: u32,
    pub baseline_name: String,
    pub baseline_sql: String,
    pub baseline_checksum: String,
    pub source_versions: Vec<u32>,
    pub already_applied_versions: Vec<u32>,
    pub applied: bool,
}

fn baseline_checksum(sql: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sql.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn collect_squash_range<'m>(
    ext_id: &str,
    migrations: &'m [Migration],
    through: u32,
) -> Result<Vec<&'m Migration>, LoaderError> {
    if through == 0 {
        return Err(LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: "--through must be >= 1; version 0 is reserved for the squash baseline"
                .to_owned(),
        });
    }

    let to_squash: Vec<&Migration> = migrations
        .iter()
        .filter(|m| m.version >= 1 && m.version <= through)
        .collect();

    if to_squash.is_empty() {
        return Err(LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "No migrations in range 1..={through} are defined for extension '{ext_id}'"
            ),
        });
    }

    let mut covered: Vec<u32> = to_squash.iter().map(|m| m.version).collect();
    covered.sort_unstable();
    if covered != (1..=through).collect::<Vec<u32>>() {
        return Err(LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "Migrations 1..={through} are not contiguous for extension '{ext_id}': have \
                 {covered:?}"
            ),
        });
    }

    Ok(to_squash)
}

fn build_baseline_sql(to_squash: &[&Migration]) -> String {
    let mut baseline_sql = String::new();
    for m in to_squash {
        baseline_sql.push_str(&format!(
            "-- migration {ver:03}: {name}\n",
            ver = m.version,
            name = m.name
        ));
        baseline_sql.push_str(m.sql);
        if !m.sql.ends_with('\n') {
            baseline_sql.push('\n');
        }
        baseline_sql.push('\n');
    }
    baseline_sql
}

impl MigrationService<'_> {
    pub async fn squash_through(
        &self,
        extension: &dyn Extension,
        through: u32,
        apply: bool,
    ) -> Result<SquashPlan, LoaderError> {
        let ext_id = extension.metadata().id;

        let mut migrations = extension.migrations();
        migrations.sort_by_key(|m| m.version);
        let to_squash = collect_squash_range(ext_id, &migrations, through)?;

        let baseline_sql = build_baseline_sql(&to_squash);
        let checksum = baseline_checksum(&baseline_sql);
        let baseline_name = format!("baseline_v{through}");
        let covered: Vec<u32> = to_squash.iter().map(|m| m.version).collect();

        self.verify_range_applied(ext_id, through).await?;

        let plan = SquashPlan {
            extension_id: ext_id.to_owned(),
            through,
            baseline_name: baseline_name.clone(),
            baseline_sql,
            baseline_checksum: checksum.clone(),
            source_versions: covered,
            already_applied_versions: (1..=through).collect(),
            applied: false,
        };

        if !apply {
            return Ok(plan);
        }

        self.apply_squash_rows(ext_id, through, &baseline_name, &checksum)
            .await?;

        Ok(SquashPlan {
            applied: true,
            ..plan
        })
    }

    async fn verify_range_applied(&self, ext_id: &str, through: u32) -> Result<(), LoaderError> {
        self.ensure_migrations_table_exists().await?;
        let applied = self.get_applied_migrations(ext_id).await?;
        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();
        let not_applied: Vec<u32> = (1..=through)
            .filter(|v| !applied_versions.contains(v))
            .collect();
        if not_applied.is_empty() {
            return Ok(());
        }
        Err(LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "Refusing to squash through {through}: extension '{ext_id}' has not applied \
                 migrations {not_applied:?}. Squashing would retire them behind the baseline \
                 without ever running them. Apply migrations 1..={through} first."
            ),
        })
    }

    async fn apply_squash_rows(
        &self,
        ext_id: &str,
        through: u32,
        baseline_name: &str,
        checksum: &str,
    ) -> Result<(), LoaderError> {
        let baseline_id = format!("{ext_id}_000");

        self.db
            .execute(
                &"INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
                  VALUES ($1, $2, 0, $3, $4) ON CONFLICT (extension_id, version) DO UPDATE SET \
                  name = EXCLUDED.name, checksum = EXCLUDED.checksum",
                &[&baseline_id, &ext_id, &baseline_name, &checksum],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("Failed to record baseline migration row: {e}"),
            })?;

        self.db
            .execute(
                &"DELETE FROM extension_migrations WHERE extension_id = $1 AND version BETWEEN 1 \
                  AND $2",
                &[&ext_id, &through],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("Failed to retire squashed migration rows: {e}"),
            })?;

        info!(
            extension = %ext_id,
            through,
            baseline_name = %baseline_name,
            "Squash applied: baseline row inserted, source rows retired"
        );

        Ok(())
    }
}
