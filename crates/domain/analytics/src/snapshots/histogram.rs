//! Version one geometric microsecond bounds merge bucket counts without
//! averaging percentiles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
/// Versioned geometric latency buckets whose counts can be merged exactly.
pub struct LatencyHistogram {
    pub version: u32,
    pub buckets: BTreeMap<u32, i64>,
}
impl Default for LatencyHistogram {
    fn default() -> Self {
        Self {
            version: 1,
            buckets: BTreeMap::new(),
        }
    }
}
impl LatencyHistogram {
    pub fn record(&mut self, micros: u64) -> crate::Result<()> {
        let bucket = if micros == 0 {
            0
        } else {
            64 - micros.leading_zeros()
        };
        let count = self.buckets.entry(bucket).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| super::invalid("Histogram overflow"))?;
        Ok(())
    }
    pub fn merge(&mut self, other: &Self) -> crate::Result<()> {
        if self.version != 1 || other.version != 1 {
            return Err(super::invalid("Unsupported histogram version"));
        }
        for (bucket, count) in &other.buckets {
            if *bucket > 64 || *count < 0 {
                return Err(super::invalid("Invalid histogram bucket"));
            }
            let current = self.buckets.entry(*bucket).or_default();
            *current = current
                .checked_add(*count)
                .ok_or_else(|| super::invalid("Histogram overflow"))?;
        }
        Ok(())
    }
    pub fn percentile_upper_bound_micros(&self, percentile: u32) -> crate::Result<Option<u64>> {
        if !(1..=100).contains(&percentile) || self.version != 1 {
            return Err(super::invalid("Invalid histogram percentile or version"));
        }
        let count = self
            .buckets
            .values()
            .try_fold(0i64, |sum, value| sum.checked_add(*value))
            .ok_or_else(|| super::invalid("Histogram overflow"))?;
        if count == 0 {
            return Ok(None);
        }
        let target = (i128::from(count) * i128::from(percentile) + 99) / 100;
        let mut running = 0i128;
        for (bucket, count) in &self.buckets {
            if *count < 0 || *bucket > 64 {
                return Err(super::invalid("Invalid histogram"));
            }
            running += i128::from(*count);
            if running >= target {
                return Ok(Some(if *bucket == 0 {
                    0
                } else {
                    1u64.checked_shl(*bucket)
                        .map_or(u64::MAX, |value| value - 1)
                }));
            }
        }
        Err(super::invalid("Invalid histogram count"))
    }
}
