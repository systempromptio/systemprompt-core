use std::path::Path;
use systemprompt_models::profile::{
    GatewayCatalog, GatewayConfig, GatewayProfileError, GatewayResult,
};

pub fn resolve_catalog(gateway: &mut GatewayConfig, profile_dir: &Path) -> GatewayResult<()> {
    let Some(rel) = gateway.catalog_path.as_ref() else {
        return Ok(());
    };
    let absolute = if rel.is_absolute() {
        rel.clone()
    } else {
        profile_dir.join(rel)
    };
    let content =
        std::fs::read_to_string(&absolute).map_err(|source| GatewayProfileError::CatalogRead {
            path: absolute.clone(),
            source,
        })?;
    let catalog: GatewayCatalog =
        serde_yaml::from_str(&content).map_err(|source| GatewayProfileError::CatalogParse {
            path: absolute.clone(),
            source,
        })?;
    catalog
        .validate()
        .map_err(|source| GatewayProfileError::CatalogInvalid {
            path: absolute.clone(),
            source: Box::new(source),
        })?;
    gateway.catalog_path = Some(absolute);
    gateway.catalog = Some(catalog);
    Ok(())
}
