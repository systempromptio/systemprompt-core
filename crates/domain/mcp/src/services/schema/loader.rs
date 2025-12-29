use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct SchemaLoader;

impl SchemaLoader {
    pub fn load_schema_file(service_path: &Path, schema_file: &str) -> Result<String> {
        let schema_path = service_path.join(schema_file);

        if !schema_path.exists() {
            anyhow::bail!(
                "Schema file not found: {} (full path: {})",
                schema_file,
                schema_path.display()
            );
        }

        let content = fs::read_to_string(&schema_path)
            .with_context(|| format!("Failed to read schema file: {}", schema_path.display()))?;

        if content.trim().is_empty() {
            anyhow::bail!("Schema file is empty: {schema_file}");
        }

        Ok(content)
    }

    pub fn list_schema_files(service_path: &Path) -> Result<Vec<PathBuf>> {
        let schema_dir = service_path.join("schema");

        if !schema_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&schema_dir).with_context(|| {
            format!("Failed to read schema directory: {}", schema_dir.display())
        })?;

        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sql"))
            .map(Ok)
            .collect()
    }

    pub fn validate_schema_syntax(sql: &str) -> Result<()> {
        let sql_upper = sql.trim().to_uppercase();

        if !sql_upper.starts_with("CREATE TABLE") && !sql_upper.starts_with("--") {
            anyhow::bail!("Schema must start with CREATE TABLE statement");
        }

        if !sql_upper.contains("CREATE TABLE") {
            anyhow::bail!("Schema must contain at least one CREATE TABLE statement");
        }

        Ok(())
    }

    pub fn validate_table_naming(sql: &str, module_name: &str) -> Result<()> {
        let module_prefix = module_name.replace('-', "_");
        let table_names = Self::extract_table_names(sql);

        if table_names.is_empty() {
            anyhow::bail!("No CREATE TABLE statements found in schema");
        }

        let invalid = table_names
            .iter()
            .find(|name| !name.starts_with(&module_prefix));

        match invalid {
            Some(table_name) => anyhow::bail!(
                "Table name '{table_name}' must start with module prefix '{module_prefix}' (from \
                 module '{module_name}')"
            ),
            None => Ok(()),
        }
    }

    fn extract_table_names(sql: &str) -> Vec<String> {
        let sql_upper = sql.to_uppercase();

        sql_upper
            .lines()
            .map(str::trim)
            .filter(|line| Self::is_create_table_line(line))
            .filter_map(Self::parse_table_name)
            .collect()
    }

    fn is_create_table_line(line: &str) -> bool {
        line.starts_with("CREATE TABLE") || line.contains("CREATE TABLE IF NOT EXISTS")
    }

    fn parse_table_name(line: &str) -> Option<String> {
        const PATTERNS: [&str; 2] = ["CREATE TABLE IF NOT EXISTS ", "CREATE TABLE "];

        PATTERNS
            .iter()
            .find_map(|pattern| line.trim().strip_prefix(pattern))
            .and_then(|after_create| after_create.split_whitespace().next())
            .map(|name| {
                name.trim_matches('(')
                    .trim_matches('"')
                    .trim_matches('`')
                    .to_string()
            })
    }
}
