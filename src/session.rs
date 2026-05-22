use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A persisted bank account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub uid: String,
    pub iban: String,
    pub name: String,
    pub currency: String,
}

/// A stored session for a single bank, including its accounts and validity window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: String,
    pub accounts: Vec<StoredAccount>,
    pub created_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
}

impl StoredSession {
    /// Returns true if the session is still within its validity period.
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.valid_until
    }
}

/// A map of bank name to its stored session, supporting multi-bank persistence.
pub type Store = HashMap<String, StoredSession>;

/// Atomically writes the session store to disk as JSON with restricted permissions.
///
/// Creates the parent directory with `0o700`, serializes with 2-space indent,
/// writes to a temporary file with `0o600`, then atomically renames to the target path.
pub fn save(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("set permissions on {}", parent.display()))?;
        }
    }

    let data = serde_json::to_string_pretty(store).context("marshal session store")?;

    let tmp_path = path.with_extension("tmp");
    let mut tmp_file = fs::File::create(&tmp_path)
        .with_context(|| format!("create temp file {}", tmp_path.display()))?;

    tmp_file
        .write_all(data.as_bytes())
        .with_context(|| format!("write temp file {}", tmp_path.display()))?;

    tmp_file
        .flush()
        .with_context(|| format!("flush temp file {}", tmp_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp_file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("set permissions on {}", tmp_path.display()))?;
    }

    fs::rename(&tmp_path, path)
        .with_context(|| format!("rename {} -> {}", tmp_path.display(), path.display()))?;

    Ok(())
}

/// Reads the session store from disk.
///
/// Returns `Ok(None)` when the file does not exist, `Ok(Some(store))` when loaded
/// successfully, or `Err(...)` on any other I/O or deserialization error.
pub fn load(path: &Path) -> Result<Option<Store>> {
    match fs::read_to_string(path) {
        Ok(data) => {
            let store: Store = serde_json::from_str(&data).context("unmarshal session store")?;
            Ok(Some(store))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}
