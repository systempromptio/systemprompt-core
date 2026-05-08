//! Pure-function diff calculators for content.
//!
//! Each calculator hashes the disk-side and database-side representations
//! and emits a structured diff (`added`/`modified`/`removed`/`unchanged`)
//! without mutating either side.

mod content;

pub use content::ContentDiffCalculator;

use sha2::{Digest, Sha256};

pub fn compute_content_hash(body: &str, title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}
