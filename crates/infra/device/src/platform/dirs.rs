//! Base directories and the single data root.
//!
//! A product keeps everything it writes for a user under one root,
//! `<per-user base>/<vendor>/<product>[ channel]`, with one directory per
//! kind below it: `data`, `config`, `cache`, `state` and `tmp`. The vendor,
//! product and channel come from the product's brand, so two brands and
//! several channels install side by side. The per-user and machine bases are
//! the platform's; everything here is derived from an [`Environment`], which
//! is the process's own unless a caller supplies one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::platform::{Native, Platform};
use crate::secure::{SecureError, check_segment, ensure_private_dir};

/// Why a base directory or a data root could not be named.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DirsError {
    #[error("no home directory is known for the current user")]
    NoHome,
    #[error("{variable} does not name an absolute directory")]
    NotAbsolute { variable: &'static str },
    #[error("{value:?} cannot name a directory: {reason}")]
    Name { value: String, reason: &'static str },
}

/// The facts base directories are derived from: the user's home directory and
/// the environment variables the platform defines.
pub struct Environment<'a> {
    home: Option<PathBuf>,
    variable: &'a dyn Fn(&str) -> Option<OsString>,
}

impl fmt::Debug for Environment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment")
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}

fn process_variable(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

impl Environment<'static> {
    #[must_use]
    pub fn process() -> Self {
        Self {
            home: std::env::home_dir(),
            variable: &process_variable,
        }
    }
}

impl<'a> Environment<'a> {
    #[must_use]
    pub const fn new(
        home: Option<PathBuf>,
        variable: &'a dyn Fn(&str) -> Option<OsString>,
    ) -> Self {
        Self { home, variable }
    }

    pub fn home(&self) -> Result<&Path, DirsError> {
        self.home
            .as_deref()
            .filter(|home| home.is_absolute())
            .ok_or(DirsError::NoHome)
    }

    pub fn directory(&self, variable: &'static str) -> Result<PathBuf, DirsError> {
        (self.variable)(variable)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or(DirsError::NotAbsolute { variable })
    }
}

pub fn home_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
    environment.home().map(Path::to_path_buf)
}

pub fn local_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
    Native::local_data_dir(environment)
}

pub fn machine_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
    Native::machine_data_dir(environment)
}

/// The directory names of one product below a base directory: the vendor,
/// then the product with its channel, if it has one, after a space.
///
/// Each part must be a file name every supported system accepts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RootName {
    vendor: String,
    product: String,
}

impl RootName {
    pub fn new(vendor: &str, product: &str, channel: Option<&str>) -> Result<Self, DirsError> {
        let vendor = part(vendor)?;
        let product = part(product)?;
        let product = match channel {
            Some(channel) => format!("{product} {}", part(channel)?),
            None => product.to_owned(),
        };
        Ok(Self {
            vendor: vendor.to_owned(),
            product,
        })
    }

    #[must_use]
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    #[must_use]
    pub fn product(&self) -> &str {
        &self.product
    }

    fn under(&self, base: &Path) -> PathBuf {
        base.join(&self.vendor).join(&self.product)
    }
}

fn part(value: &str) -> Result<&str, DirsError> {
    if value.contains('/') {
        return Err(DirsError::Name {
            value: value.to_owned(),
            reason: "the name holds a path separator",
        });
    }
    match check_segment(value) {
        Ok(()) if value.trim() == value => Ok(value),
        Ok(()) => Err(DirsError::Name {
            value: value.to_owned(),
            reason: "the name starts with a space",
        }),
        Err(reason) => Err(DirsError::Name {
            value: value.to_owned(),
            reason,
        }),
    }
}

/// The single data root of one product for the current user, and the
/// directory of each kind below it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataRoot {
    root: PathBuf,
}

impl DataRoot {
    pub fn for_user(environment: &Environment<'_>, name: &RootName) -> Result<Self, DirsError> {
        local_data_dir(environment).map(|base| Self::at(name.under(&base)))
    }

    #[must_use]
    pub const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    #[must_use]
    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    #[must_use]
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn ensure(&self) -> Result<(), SecureError> {
        ensure_private_dir(&self.root)?;
        for dir in [
            self.data(),
            self.config(),
            self.cache(),
            self.state(),
            self.tmp(),
        ] {
            ensure_private_dir(&dir)?;
        }
        Ok(())
    }
}
