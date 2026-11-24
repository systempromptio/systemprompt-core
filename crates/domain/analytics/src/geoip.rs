//! The configured `GeoIP` database the analytics member opens once and
//! provides as [`GeoIpDatabase`].
//!
//! An unset `geoip_database_path` is no database: geographic enrichment is
//! off and a startup line says so. A configured path that cannot be opened
//! fails the provide, so the host refuses to boot over a misconfiguration the
//! operator asked for. Without the `geolocation` feature no database is ever
//! opened.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::GeoIpReader;

/// The `GeoIP` database the analytics services enrich sessions from; empty
/// when none is configured.
#[derive(Clone, Default)]
pub struct GeoIpDatabase {
    reader: Option<GeoIpReader>,
}

#[derive(Debug, thiserror::Error)]
#[error("GeoIP database at {path} could not be opened")]
pub struct GeoIpUnreadable {
    pub path: String,
    #[source]
    pub source: Box<dyn std::error::Error + Send + Sync>,
}

impl GeoIpDatabase {
    #[must_use]
    pub const fn new(reader: Option<GeoIpReader>) -> Self {
        Self { reader }
    }

    #[must_use]
    pub const fn reader(&self) -> Option<&GeoIpReader> {
        self.reader.as_ref()
    }

    #[cfg(feature = "geolocation")]
    pub fn open(path: Option<&str>) -> Result<Self, GeoIpUnreadable> {
        let Some(path) = path else {
            tracing::warn!(
                "GeoIP database not configured (paths.geoip_database): geographic data will not \
                 be available"
            );
            return Ok(Self::default());
        };
        maxminddb::Reader::open_readfile(path)
            .map(|reader| Self::new(Some(std::sync::Arc::new(reader))))
            .map_err(|source| GeoIpUnreadable {
                path: path.to_owned(),
                source: Box::new(source),
            })
    }

    #[cfg(not(feature = "geolocation"))]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "mirrors the geolocation opener so the provide has one shape"
    )]
    pub fn open(_path: Option<&str>) -> Result<Self, GeoIpUnreadable> {
        Ok(Self::default())
    }
}

impl std::fmt::Debug for GeoIpDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeoIpDatabase")
            .field("configured", &self.reader.is_some())
            .finish()
    }
}
