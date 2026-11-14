//! Enumeration of every link-time registration seam compiled into this
//! binary.
//!
//! Each seam is a [`LinkedSeamRegistration`] submitted with
//! [`register_linked_seam!`](crate::register_linked_seam) by the crate that
//! links it: the kernel's own seams are registered by this command's
//! modules, a capability's by its entry crate. A registration that carries a
//! stable name, id or tag is listed by it; one that carries only a priority
//! is listed by priority; one that carries only a factory is listed as
//! `unnamed#N`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::{LinkedSeamRegistration, registered_seams};

#[derive(Debug, Clone)]
pub struct LinkedSeam {
    pub name: &'static str,
    pub owner: &'static str,
    pub extension: Option<&'static str>,
    pub entries: Vec<String>,
}

impl LinkedSeam {
    #[must_use]
    pub fn enumerate(registration: &LinkedSeamRegistration) -> Self {
        Self {
            name: registration.name,
            owner: registration.owner,
            extension: registration.extension,
            entries: (registration.entries)(),
        }
    }
}

#[must_use]
pub fn linked_seams() -> Vec<LinkedSeam> {
    registered_seams()
        .into_iter()
        .map(LinkedSeam::enumerate)
        .collect()
}
