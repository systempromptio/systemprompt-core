//! Claims: [`admit_extensions`] judges every extension declaration
//! together, after each has passed `admit_extension`.
//!
//! Each core row of an owner that declares any router is claimed by exactly
//! one of that owner's declarations, and each declaration claims at most one
//! row (same owner, base path, group and whole shape); a row left unclaimed,
//! claimed twice, or a declaration matching two rows refuses boot with a
//! typed [`MountError`]. A disabled owner declares nothing and claims
//! nothing. A row's shape cannot see which handlers its router holds, so the
//! claimed routers are then bounded by route: a row with route pins
//! (`CoreDeclaration::route_pins`) confines its router to them ([`pinned`]),
//! and the other routers at that owner's base path and group answer 404
//! under a sibling's pins ([`excluding`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::registry::ApiRouter;
use systemprompt_extension::{ExtensionRouter, RouteGroup};

use super::MountError;
use super::core_declaration::{CoreDeclaration, CoreDeclarer, admit_extension};
use super::core_pins::{excluding, pinned};

fn owner_rows(owner: CoreDeclarer) -> impl Iterator<Item = CoreDeclaration> {
    CoreDeclaration::ALL
        .iter()
        .copied()
        .filter(move |row| row.declarer() == owner && row.group() != RouteGroup::Gateway)
}

fn claimed_row(
    extension: &'static str,
    declared: &ExtensionRouter,
) -> Result<Option<CoreDeclaration>, MountError> {
    let owner = CoreDeclarer::Extension(extension);
    let mut claims = owner_rows(owner).filter(|row| {
        row.names(owner, declared.base_path, declared.group)
            && row.shape_matches(&declared.protection)
    });
    match (claims.next(), claims.next()) {
        (Some(first), Some(second)) => Err(MountError::CoreDeclarationAmbiguous {
            extension,
            base_path: declared.base_path,
            first,
            second,
        }),
        (claimed, _) => Ok(claimed),
    }
}

fn check_claims(
    declared: &[ApiRouter],
    claims: &[Option<CoreDeclaration>],
) -> Result<(), MountError> {
    for (index, (ext, _)) in declared.iter().enumerate() {
        let extension = ext.metadata().id;
        let first_of_owner = declared[..index]
            .iter()
            .all(|(earlier, _)| earlier.metadata().id != extension);
        if !first_of_owner {
            continue;
        }
        for row in owner_rows(CoreDeclarer::Extension(extension)) {
            let count = declared
                .iter()
                .zip(claims)
                .filter(|((other, _), claim)| {
                    other.metadata().id == extension && **claim == Some(row)
                })
                .count();
            match count {
                1 => {},
                0 => return Err(MountError::CoreDeclarationUnclaimed { extension, row }),
                _ => return Err(MountError::CoreDeclarationClaimedTwice { extension, row }),
            }
        }
    }
    Ok(())
}

fn sibling_pins(row: CoreDeclaration) -> Vec<&'static str> {
    owner_rows(row.declarer())
        .filter(|sibling| {
            *sibling != row
                && sibling.base_path() == row.base_path()
                && sibling.group() == row.group()
        })
        .flat_map(|sibling| sibling.route_pins().iter().map(|pin| pin.prefix))
        .collect()
}

pub fn admit_extensions(declared: &mut [ApiRouter]) -> Result<(), MountError> {
    let mut claims = Vec::with_capacity(declared.len());
    for (ext, router) in declared.iter() {
        let extension = ext.metadata().id;
        admit_extension(extension, router)?;
        claims.push(claimed_row(extension, router)?);
    }
    check_claims(declared, &claims)?;
    for ((_, router), claim) in declared.iter_mut().zip(claims) {
        let Some(row) = claim else {
            continue;
        };
        let built = std::mem::take(&mut router.router);
        router.router = if row.route_pins().is_empty() {
            let claimed = sibling_pins(row);
            if claimed.is_empty() {
                built
            } else {
                excluding(built, claimed)
            }
        } else {
            pinned(built, row.route_pins())
        };
    }
    Ok(())
}
