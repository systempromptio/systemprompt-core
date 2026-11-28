//! The parity ledger: one row per scenario, protocol and comparison, one entry
//! per difference, each classified from the reviewed file
//! (`classifications.toml` beside this crate) in the vocabulary of
//! `testing/03-golden-scenarios.md` section 6. A difference the file does not
//! name is `unclassified`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compare::Difference;

pub const CLASSES: [&str; 6] = [
    "same",
    "intended",
    "gap",
    "defect",
    "reference-defect",
    "unclassified",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    pub scenario: String,
    #[serde(default = "every")]
    pub protocol: String,
    #[serde(default = "harness_vs_expected")]
    pub comparison: String,
    pub path: String,
    pub class: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub evidence: Option<String>,
}

fn every() -> String {
    "*".to_owned()
}

fn harness_vs_expected() -> String {
    HARNESS_VS_EXPECTED.to_owned()
}

pub const HARNESS_VS_EXPECTED: &str = "harness-vs-expected";
pub const HARNESS_VS_REFERENCE: &str = "harness-vs-reference";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classifications {
    #[serde(default)]
    pub entry: Vec<Classification>,
}

impl Classifications {
    pub const FILE: &'static str = "classifications.toml";

    /// The reviewed file of this crate.
    pub fn reviewed() -> anyhow::Result<Self> {
        Self::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join(Self::FILE))
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let parsed: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        parsed.check()?;
        Ok(parsed)
    }

    /// Every entry uses the vocabulary and carries what its class requires.
    pub fn check(&self) -> anyhow::Result<()> {
        for entry in &self.entry {
            let what = format!("{} {} {}", entry.scenario, entry.protocol, entry.path);
            anyhow::ensure!(
                CLASSES.contains(&entry.class.as_str())
                    && entry.class != "same"
                    && entry.class != "unclassified",
                "{what}: class {} is not one a reviewed entry may carry",
                entry.class
            );
            anyhow::ensure!(!entry.reason.trim().is_empty(), "{what}: no reason");
            anyhow::ensure!(
                entry.class != "gap" || entry.owner.is_some(),
                "{what}: a gap needs an owner stage"
            );
            anyhow::ensure!(
                entry.class != "reference-defect" || entry.evidence.is_some(),
                "{what}: a reference-defect needs its evidence"
            );
        }
        Ok(())
    }

    pub fn find(
        &self,
        scenario: &str,
        protocol: &str,
        comparison: &str,
        path: &str,
    ) -> Option<&Classification> {
        self.entry.iter().find(|entry| {
            entry.scenario == scenario
                && (entry.protocol == "*" || entry.protocol == protocol)
                && entry.comparison == comparison
                && entry
                    .path
                    .strip_suffix('*')
                    .map_or(entry.path == path, |prefix| path.starts_with(prefix))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    pub difference: Difference,
    pub class: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRow {
    pub scenario: String,
    pub protocol: String,
    pub comparison: String,
    pub result: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub entries: Vec<Entry>,
}

impl LedgerRow {
    pub fn classify(
        scenario: &str,
        protocol: &str,
        comparison: &str,
        found: Vec<Difference>,
        reviewed: &Classifications,
    ) -> Self {
        let entries: Vec<Entry> = found
            .into_iter()
            .map(|difference| {
                let known = reviewed.find(scenario, protocol, comparison, &difference.path);
                Entry {
                    class: known
                        .map_or_else(|| "unclassified".to_owned(), |entry| entry.class.clone()),
                    reason: known.map(|entry| entry.reason.clone()).unwrap_or_default(),
                    difference,
                }
            })
            .collect();
        Self {
            scenario: scenario.to_owned(),
            protocol: protocol.to_owned(),
            comparison: comparison.to_owned(),
            result: if entries.is_empty() {
                "same"
            } else {
                "differs"
            }
            .to_owned(),
            error: None,
            entries,
        }
    }

    pub fn failed(scenario: &str, protocol: &str, comparison: &str, error: String) -> Self {
        Self {
            scenario: scenario.to_owned(),
            protocol: protocol.to_owned(),
            comparison: comparison.to_owned(),
            result: "error".to_owned(),
            error: Some(error),
            entries: Vec::new(),
        }
    }

    pub fn unclassified(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.class == "unclassified")
            .collect()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub rows: Vec<LedgerRow>,
}

impl Ledger {
    /// Rows by result, and differences by class (`same` counts the rows with
    /// none).
    pub fn counts(&self) -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = CLASSES
            .iter()
            .map(|class| ((*class).to_owned(), 0))
            .collect();
        for row in &self.rows {
            if row.result == "same" {
                *counts.entry("same".to_owned()).or_default() += 1;
            }
            if row.result == "error" {
                *counts.entry("error".to_owned()).or_default() += 1;
            }
            for entry in &row.entries {
                *counts.entry(entry.class.clone()).or_default() += 1;
            }
        }
        counts
    }

    pub fn markdown(&self) -> String {
        let mut text = String::from("# Parity ledger\n\n| Class | Count |\n|---|---|\n");
        for (class, count) in self.counts() {
            text.push_str(&format!("| {class} | {count} |"));
            text.push('\n');
        }
        text.push_str("\n| Scenario | Protocol | Comparison | Result | Differences |\n|---|---|---|---|---|\n");
        for row in &self.rows {
            text.push_str(&format!(
                "| {} | {} | {} | {} | {} |",
                row.scenario,
                row.protocol,
                row.comparison,
                row.result,
                row.entries.len()
            ));
            text.push('\n');
        }
        text.push_str("\n## Differences\n");
        for row in self.rows.iter().filter(|row| row.result != "same") {
            text.push_str(&format!(
                "\n### {} / {} / {}\n",
                row.scenario, row.protocol, row.comparison
            ));
            text.push('\n');
            if let Some(error) = &row.error {
                text.push_str(&format!("- error: {error}"));
                text.push('\n');
            }
            for entry in &row.entries {
                text.push_str(&format!(
                    "- `{}` [{}] expected `{}`, got `{}`{}",
                    entry.difference.path,
                    entry.class,
                    entry.difference.expected,
                    entry.difference.actual,
                    if entry.reason.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", entry.reason)
                    }
                ));
                text.push('\n');
            }
        }
        text
    }

    pub fn write(&self, dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("ledger.json"), serde_json::to_string_pretty(self)?)?;
        std::fs::write(dir.join("ledger.md"), self.markdown())?;
        Ok(())
    }
}
