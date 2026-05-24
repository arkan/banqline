// Configuration management for Banqline.
//
// Mirrors internal/config/config.go from the Go codebase.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::alerter::types::AlertRule;
use crate::tagger::CategoryRule;

// ---------------------------------------------------------------------------
// TagRules — custom YAML (de)serialization
//
// The Go config stores tag rules as a YAML mapping (category → patterns).
// TagRules bridges that map-based representation to the internal Vec<CategoryRule>.
// ---------------------------------------------------------------------------

/// A sequence of category-to-patterns rules that serializes as a YAML mapping.
///
/// ```yaml
/// groceries: [carrefour, auchan]
/// transport:  [sncf, ratp]
/// ```
#[derive(Debug, Clone, Default)]
pub struct TagRules(pub Vec<CategoryRule>);

impl TagRules {
    /// Returns `true` when the rules list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for TagRules {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map: BTreeMap<&str, &[String]> = self
            .0
            .iter()
            .map(|r| (r.category.as_str(), r.patterns.as_slice()))
            .collect();
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagRules {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Accept null/absent → empty; mapping → convert pairs.
        // IndexMap preserves YAML document order (alphabetical from BTreeMap serialization).
        let opt: Option<IndexMap<String, Vec<String>>> = Option::deserialize(deserializer)?;
        Ok(TagRules(match opt {
            None => Vec::new(),
            Some(map) => map
                .into_iter()
                .map(|(category, patterns)| CategoryRule { category, patterns })
                .collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Application configuration, backed by a YAML file at
/// `~/.config/banqline/config.yaml` by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Enable Banking application ID (OAuth `client_id`).
    #[serde(rename = "application_id", default)]
    pub application_id: String,

    /// Path to the PEM-encoded RSA private key.
    #[serde(rename = "key_path", default)]
    pub key_path: String,

    /// OAuth redirect URL.
    #[serde(rename = "redirect_url", default = "default_redirect_url")]
    pub redirect_url: String,

    /// Local port for the OAuth callback server.
    #[serde(rename = "callback_port", default = "default_callback_port")]
    pub callback_port: u16,

    /// Default bank name used when `--bank` is omitted.
    #[serde(rename = "default_bank", default)]
    pub default_bank: String,

    /// Log level: `trace`, `debug`, `info`, `warn`, `error`.
    #[serde(rename = "log_level", default = "default_log_level")]
    pub log_level: String,

    /// Category-to-patterns mapping for transaction tagging.
    #[serde(
        rename = "tag_rules",
        default,
        skip_serializing_if = "TagRules::is_empty"
    )]
    pub tag_rules: TagRules,

    /// Alert rule definitions.
    #[serde(rename = "alert_rules", default, skip_serializing_if = "Vec::is_empty")]
    pub alert_rules: Vec<AlertRule>,
}

// --- Serde default helpers ---

fn default_redirect_url() -> String {
    "http://localhost:19876/callback".into()
}

fn default_callback_port() -> u16 {
    19876
}

fn default_app_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_default()
        .join("banqline")
}

fn default_config_path() -> PathBuf {
    default_app_dir().join("config.yaml")
}

fn default_log_level() -> String {
    "info".into()
}

// --- Constructor ---

/// Returns a `Config` populated with sensible defaults.
///
/// Equivalent to `DefaultConfig()` in the Go codebase.
pub fn default_config() -> Config {
    Config {
        application_id: String::new(),
        key_path: String::new(),
        redirect_url: default_redirect_url(),
        callback_port: default_callback_port(),
        default_bank: String::new(),
        log_level: default_log_level(),
        tag_rules: TagRules::default(),
        alert_rules: Vec::new(),
    }
}

impl Default for Config {
    fn default() -> Self {
        default_config()
    }
}

// --- I/O ---

impl Config {
    /// Loads a `Config` from the given YAML file path.
    ///
    /// Fields absent from the file are filled with defaults (see
    /// [`default_config`]).
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path).context("read config")?;
        serde_yaml::from_str(&data).context("parse config")
    }

    /// Saves `self` to `path` as YAML with restricted permissions.
    ///
    /// Creates parent directories with `0o700` and writes the file with
    /// `0o600` (Unix).  On non-Unix platforms the permission hardening is
    /// skipped.
    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir).with_context(|| format!("create dir {}", dir.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .with_context(|| format!("set permissions on {}", dir.display()))?;
        }

        let yaml = serde_yaml::to_string(self).context("marshal config")?;
        let tmp_path = path.with_extension("yaml.tmp");
        let mut tmp_file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("create temp file {}", tmp_path.display()))?;

        tmp_file
            .write_all(yaml.as_bytes())
            .with_context(|| format!("write temp file {}", tmp_path.display()))?;
        tmp_file
            .flush()
            .with_context(|| format!("flush temp file {}", tmp_path.display()))?;
        tmp_file
            .sync_all()
            .with_context(|| format!("sync temp file {}", tmp_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp_file
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("set permissions on {}", tmp_path.display()))?;
        }

        drop(tmp_file);
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("rename {} -> {}", tmp_path.display(), path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Returns the expected path of the config YAML file.
    pub fn config_path(&self) -> PathBuf {
        default_config_path()
    }

    /// Returns Banqline's fixed application data directory.
    pub fn app_dir(&self) -> PathBuf {
        default_app_dir()
    }

    /// Returns the expected path of the session JSON file inside the data
    /// directory.
    pub fn session_path(&self) -> PathBuf {
        self.app_dir().join("session.json")
    }

    /// Returns the expected path of the local SQLite database.
    pub fn data_path(&self) -> PathBuf {
        self.app_dir().join("data.db")
    }

    /// Resolves `key_path` to a file inside Banqline's fixed app directory.
    ///
    /// Only the file name from `key_path` is used, so `private.key`,
    /// `./private.key`, and `/old/location/private.key` all resolve to
    /// `~/.config/banqline/private.key` on a typical Linux system.
    /// Returns an error when `key_path` is empty or does not contain a file name.
    pub fn key_abs_path(&self) -> Result<PathBuf> {
        if self.key_path.is_empty() {
            anyhow::bail!("key_path is not set");
        }

        let file_name = Path::new(&self.key_path)
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| anyhow::anyhow!("key_path must contain a file name"))?;

        Ok(self.app_dir().join(file_name))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = default_config();
        assert_eq!(cfg.redirect_url, "http://localhost:19876/callback");
        assert_eq!(cfg.callback_port, 19876);
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.application_id.is_empty());
        assert!(cfg.key_path.is_empty());
        assert!(cfg.tag_rules.is_empty());
        assert!(cfg.alert_rules.is_empty());
    }

    #[test]
    fn config_path_helper() {
        let cfg = default_config();
        let app_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .unwrap_or_default()
            .join("banqline");

        assert_eq!(cfg.config_path(), app_dir.join("config.yaml"));
        assert_eq!(cfg.session_path(), app_dir.join("session.json"));
    }

    #[test]
    fn load_ignores_legacy_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "application_id: my-app\ndata_dir: /tmp/legacy\n").unwrap();

        let cfg = Config::load(&path).unwrap();

        assert_eq!(cfg.application_id, "my-app");
        assert_eq!(
            cfg.session_path(),
            dirs::config_dir()
                .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
                .unwrap_or_default()
                .join("banqline")
                .join("session.json")
        );
    }

    #[test]
    fn key_abs_path_empty_is_error() {
        let cfg = default_config();
        assert!(cfg.key_abs_path().is_err());
    }

    #[test]
    fn key_abs_path_resolves_relative_to_app_config_dir() {
        let app_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .unwrap_or_default()
            .join("banqline");
        let cfg = Config {
            key_path: "my-key.pem".into(),
            ..default_config()
        };
        let resolved = cfg.key_abs_path().unwrap();
        assert_eq!(resolved, app_dir.join("my-key.pem"));
    }

    #[test]
    fn key_abs_path_uses_file_name_inside_app_config_dir() {
        let app_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .unwrap_or_default()
            .join("banqline");
        let cfg = Config {
            key_path: "/tmp/other/place/my-key.pem".into(),
            ..default_config()
        };

        let resolved = cfg.key_abs_path().unwrap();

        assert_eq!(resolved, app_dir.join("my-key.pem"));
    }

    #[test]
    fn tag_rules_serialization_roundtrip() {
        let rules = TagRules(vec![
            CategoryRule {
                category: "groceries".into(),
                patterns: vec!["carrefour".into(), "auchan".into()],
            },
            CategoryRule {
                category: "transport".into(),
                patterns: vec!["sncf".into()],
            },
        ]);
        let yaml = serde_yaml::to_string(&rules).unwrap();
        let back: TagRules = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.0.len(), 2);
        assert_eq!(back.0[0].category, "groceries");
        assert_eq!(back.0[0].patterns, vec!["carrefour", "auchan"]);
    }

    #[test]
    fn tag_rules_deserialize_empty_yields_empty() {
        let yaml = "null";
        let rules: TagRules = serde_yaml::from_str(yaml).unwrap();
        assert!(rules.is_empty());

        let rules: TagRules = serde_yaml::from_str("{}").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn load_merges_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        // Write minimal YAML — absent fields should get defaults.
        std::fs::write(&path, "application_id: my-app\nlog_level: debug\n").unwrap();

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.application_id, "my-app");
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.redirect_url, "http://localhost:19876/callback");
        assert_eq!(cfg.callback_port, 19876);
    }

    #[test]
    fn save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.yaml");

        let cfg = Config {
            application_id: "test-app".into(),
            log_level: "trace".into(),
            ..default_config()
        };

        cfg.save(&path).unwrap();
        assert!(!path.with_extension("yaml.tmp").exists());

        let reloaded = Config::load(&path).unwrap();
        assert_eq!(reloaded.application_id, "test-app");
        assert_eq!(reloaded.log_level, "trace");
        assert_eq!(reloaded.callback_port, 19876);
    }

    #[test]
    fn tag_rules_in_config_roundtrip() {
        let cfg = Config {
            tag_rules: TagRules(vec![CategoryRule {
                category: "food".into(),
                patterns: vec!["lidl".into()],
            }]),
            ..default_config()
        };

        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let back: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.tag_rules.0.len(), 1);
        assert_eq!(back.tag_rules.0[0].category, "food");
        assert!(!yaml.contains("data_dir"));
    }
}
