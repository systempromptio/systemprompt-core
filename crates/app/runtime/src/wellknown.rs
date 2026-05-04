//! Static metadata for `/.well-known/` routes registered with
//! [`register_wellknown_route!`](crate::register_wellknown_route).

/// Human-readable metadata for a registered well-known route.
#[derive(Debug, Clone, Copy)]
pub struct WellKnownMetadata {
    /// Path under `/.well-known/`.
    pub path: &'static str,
    /// Short display name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
}

inventory::collect!(WellKnownMetadata);

impl WellKnownMetadata {
    /// Construct a metadata entry. Used by the registration macro.
    pub const fn new(path: &'static str, name: &'static str, description: &'static str) -> Self {
        Self {
            path,
            name,
            description,
        }
    }
}

/// Look up registered metadata for `path`, if any.
pub fn get_wellknown_metadata(path: &str) -> Option<WellKnownMetadata> {
    inventory::iter::<WellKnownMetadata>
        .into_iter()
        .find(|meta| meta.path == path)
        .copied()
}
