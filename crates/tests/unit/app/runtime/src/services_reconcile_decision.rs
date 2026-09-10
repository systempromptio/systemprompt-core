use std::collections::BTreeMap;
use std::path::PathBuf;

use systemprompt_loader::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_runtime::services_reconcile::pending_composed_hash;

fn root(provenance: ServicesProvenance) -> ActiveServicesRoot {
    ActiveServicesRoot {
        path: PathBuf::from("/app/services-cache/current"),
        provenance,
    }
}

fn fetched(hash: &str) -> ServicesProvenance {
    ServicesProvenance::Fetched {
        composed_hash: hash.to_owned(),
        versions: BTreeMap::new(),
    }
}

#[test]
fn a_fetched_composition_not_yet_reconciled_is_pending() {
    let active = root(fetched("abcdef0123456789"));
    assert_eq!(
        pending_composed_hash(&active, None),
        Some("abcdef0123456789"),
        "a freshly fetched composition with no recorded reconcile must be projected"
    );
}

#[test]
fn a_fetched_composition_already_reconciled_is_not_pending() {
    let active = root(fetched("abcdef0123456789"));
    assert_eq!(
        pending_composed_hash(&active, Some("abcdef0123456789")),
        None,
        "restarting on an already-reconciled composition must not re-project"
    );
}

#[test]
fn a_changed_composition_is_pending_again() {
    let active = root(fetched("1111111111111111"));
    assert_eq!(
        pending_composed_hash(&active, Some("abcdef0123456789")),
        Some("1111111111111111"),
        "a swapped composition must be projected even though an earlier one was"
    );
}

#[test]
fn a_baked_tree_is_never_pending() {
    let active = root(ServicesProvenance::Bundled);
    assert_eq!(
        pending_composed_hash(&active, None),
        None,
        "the tree baked into the image is reconciled by the operator, not at boot"
    );
}

#[test]
fn a_last_good_composition_is_never_pending() {
    let active = root(ServicesProvenance::LastGood {
        composed_hash: "abcdef0123456789".to_owned(),
        error: "fetch failed".to_owned(),
    });
    assert_eq!(
        pending_composed_hash(&active, None),
        None,
        "a failed refresh must not re-project the composition it fell back to"
    );
}

#[test]
fn a_bundled_fallback_is_never_pending() {
    let active = root(ServicesProvenance::BundledFallback {
        error: "fetch failed".to_owned(),
    });
    assert_eq!(pending_composed_hash(&active, None), None);
}
