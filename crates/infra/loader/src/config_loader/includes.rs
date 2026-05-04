use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ConfigLoadError, ConfigLoadResult};

use super::merge::{merge_partial, resolve_partial_includes};
use super::types::{IncludeResolveCtx, PartialServicesFile};

pub(super) fn resolve_includes_recursively(
    base_path: &Path,
    include_path: &str,
    referrer: &Path,
    ctx: &mut IncludeResolveCtx<'_>,
) -> ConfigLoadResult<()> {
    let referrer_dir = referrer.parent().unwrap_or(base_path);
    let full_path = referrer_dir.join(include_path);

    if !full_path.exists() {
        return Err(ConfigLoadError::IncludeNotFound {
            include: full_path,
            referrer: referrer.to_path_buf(),
        });
    }

    let canonical = fs::canonicalize(&full_path).map_err(|e| ConfigLoadError::Io {
        path: full_path.clone(),
        source: e,
    })?;

    if ctx.visited.contains(&canonical) {
        let mut chain: Vec<String> = ctx.chain.iter().map(|p| p.display().to_string()).collect();
        chain.push(canonical.display().to_string());
        return Err(ConfigLoadError::IncludeCycle {
            chain: chain.join(" -> "),
        });
    }
    ctx.visited.insert(canonical.clone());

    let content = fs::read_to_string(&canonical).map_err(|e| ConfigLoadError::Io {
        path: canonical.clone(),
        source: e,
    })?;

    let partial_file: PartialServicesFile =
        serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
            path: canonical.clone(),
            source: e,
        })?;

    ctx.chain.push(canonical.clone());
    for nested in &partial_file.includes {
        resolve_includes_recursively(base_path, nested, &canonical, ctx)?;
    }
    ctx.chain.pop();

    let file_dir: PathBuf = canonical.parent().unwrap_or(base_path).to_path_buf();
    let mut partial = partial_file.into_partial_config();
    resolve_partial_includes(&mut partial, &file_dir)?;
    merge_partial(ctx.merged, partial)?;

    Ok(())
}
