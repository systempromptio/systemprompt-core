//! Pure JSON manipulation for the two registry files Cowork reads:
//! `known_marketplaces.json` and `installed_plugins.json`.
//!
//! Shapes match the current Cowork (Claude 1.5354.0.0) reader, which calls
//! `Object.entries(...)` on the value at `installedPlugins` / on the root
//! object of `known_marketplaces.json`. Foreign sibling entries (other
//! marketplaces, other plugins) MUST be preserved — Cowork users may have
//! registered their own marketplaces alongside ours.
//!
//! `known_marketplaces.json` shape:
//! ```json
//! {
//!   "<marketplace-name>": {
//!     "source":         { "source": "local", "path": "<abs>" },
//!     "installLocation": "<abs>",
//!     "lastUpdated":     "<ISO8601>"
//!   }
//! }
//! ```
//!
//! `installed_plugins.json` shape:
//! ```json
//! {
//!   "version": 2,
//!   "plugins": {
//!     "<plugin>@<marketplace>": [
//!       {
//!         "scope":       "user",
//!         "installPath": "<abs>",
//!         "version":     "<v>",
//!         "installedAt": "<ISO8601>",
//!         "lastUpdated": "<ISO8601>"
//!       }
//!     ]
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::CoworkPluginsError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSource {
    // Cowork uses the field name `source` as the discriminator (not `type`).
    pub source: String,
    pub path: String,
}

impl LocalSource {
    #[must_use]
    pub fn local(path: String) -> Self {
        Self {
            source: "local".into(),
            path,
        }
    }
}

// Wire shape of one entry-value in `known_marketplaces.json`. The marketplace
// **name** is the JSON object key, not a field; see `KnownMarketplaceEntry`
// for the in-memory pair we upsert with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownMarketplaceValue {
    pub source: LocalSource,
    #[serde(rename = "installLocation")]
    pub install_location: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
}

// Whole-file wire shape for `known_marketplaces.json` — keyed object,
// foreign entries preserved by reading/writing through the inner `Map`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct KnownMarketplacesFile {
    pub marketplaces: Map<String, Value>,
}

impl KnownMarketplacesFile {
    pub fn contains(&self, marketplace_name: &str) -> bool {
        self.marketplaces.contains_key(marketplace_name)
    }
}

// Pair (key + value) used by the upsert API. `name` becomes the JSON key;
// the rest serializes into a `KnownMarketplaceValue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownMarketplaceEntry {
    pub name: String,
    pub source: LocalSource,
    pub install_location: String,
    pub last_updated: String,
}

impl KnownMarketplaceEntry {
    pub fn to_value(&self) -> KnownMarketplaceValue {
        KnownMarketplaceValue {
            source: self.source.clone(),
            install_location: self.install_location.clone(),
            last_updated: self.last_updated.clone(),
        }
    }
}

// One install record in the array at `plugins["<plugin>@<marketplace>"]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledPluginInstall {
    pub scope: String,
    #[serde(rename = "installPath")]
    pub install_path: String,
    pub version: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
}

// Whole-file wire shape for `installed_plugins.json`. Cowork's reader calls
// `Object.entries(plugins)` and treats each value as an array of installs —
// the empty-or-missing `plugins` map is what was crashing
// `LocalPluginsReader.getAllLocalPluginsWithResolver` before the schema fix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledPluginsFile {
    pub version: u32,
    pub plugins: Map<String, Value>,
}

impl Default for InstalledPluginsFile {
    fn default() -> Self {
        Self {
            version: INSTALLED_VERSION,
            plugins: Map::new(),
        }
    }
}

// Pair (key + value) used by the upsert API. `marketplace` + `name` join to
// form the JSON key `"<name>@<marketplace>"`; the rest of the fields project
// to `InstalledPluginInstall`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPluginEntry {
    pub marketplace: String,
    pub name: String,
    pub scope: String,
    pub install_path: String,
    pub version: String,
    pub installed_at: String,
    pub last_updated: String,
}

impl InstalledPluginEntry {
    pub fn to_install(&self) -> InstalledPluginInstall {
        InstalledPluginInstall {
            scope: self.scope.clone(),
            install_path: self.install_path.clone(),
            version: self.version.clone(),
            installed_at: self.installed_at.clone(),
            last_updated: self.last_updated.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub inserted: Vec<String>,
    pub replaced: Vec<String>,
    pub unchanged: Vec<String>,
}

pub fn parse_root(bytes: &[u8]) -> Result<Map<String, Value>, CoworkPluginsError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(bytes)? {
        Value::Object(root) => Ok(root),
        _ => Err(CoworkPluginsError::RootShape),
    }
}

pub fn upsert_known_marketplace(
    root: &mut Map<String, Value>,
    entry: &KnownMarketplaceEntry,
) -> Result<MergeReport, CoworkPluginsError> {
    let new_value = serde_json::to_value(entry.to_value())?;
    let mut report = MergeReport::default();
    match root.get(&entry.name) {
        Some(existing) if existing == &new_value => report.unchanged.push(entry.name.clone()),
        Some(_) => {
            root.insert(entry.name.clone(), new_value);
            report.replaced.push(entry.name.clone());
        },
        None => {
            root.insert(entry.name.clone(), new_value);
            report.inserted.push(entry.name.clone());
        },
    }
    Ok(report)
}

pub fn retain_known_marketplaces(root: &mut Map<String, Value>, drop_name: &str) {
    root.remove(drop_name);
}

const INSTALLED_VERSION: u32 = 2;
const INSTALLED_PLUGINS_KEY: &str = "plugins";
const INSTALLED_VERSION_KEY: &str = "version";

pub fn installed_plugin_key(entry: &InstalledPluginEntry) -> String {
    format!("{}@{}", entry.name, entry.marketplace)
}

pub fn upsert_installed_plugin(
    root: &mut Map<String, Value>,
    entry: &InstalledPluginEntry,
) -> Result<MergeReport, CoworkPluginsError> {
    root.insert(
        INSTALLED_VERSION_KEY.into(),
        serde_json::to_value(INSTALLED_VERSION)?,
    );
    let plugins = ensure_object(root, INSTALLED_PLUGINS_KEY)?;
    let key = installed_plugin_key(entry);
    let new_array = serde_json::to_value(vec![entry.to_install()])?;
    let mut report = MergeReport::default();
    match plugins.get(&key) {
        Some(existing) if existing == &new_array => report.unchanged.push(key),
        Some(_) => {
            plugins.insert(key.clone(), new_array);
            report.replaced.push(key);
        },
        None => {
            plugins.insert(key.clone(), new_array);
            report.inserted.push(key);
        },
    }
    Ok(report)
}

pub fn retain_installed_plugin(root: &mut Map<String, Value>, plugin_key: &str) {
    if let Some(Value::Object(plugins)) = root.get_mut(INSTALLED_PLUGINS_KEY) {
        plugins.remove(plugin_key);
    }
}

fn ensure_object<'a>(
    root: &'a mut Map<String, Value>,
    key: &'static str,
) -> Result<&'a mut Map<String, Value>, CoworkPluginsError> {
    match root
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
    {
        Value::Object(m) => Ok(m),
        _ => Err(CoworkPluginsError::ItemsShape { key }),
    }
}

