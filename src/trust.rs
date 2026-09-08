use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TRUST_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    Pending,
    Restricted,
    Trusted,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrustRecord {
    version: u8,
    path: PathBuf,
    trusted: bool,
}

pub struct TrustStore {
    directory: PathBuf,
}

impl TrustStore {
    pub fn new() -> Result<Self> {
        let base =
            dirs::data_local_dir().context("could not determine the application data directory")?;
        Ok(Self {
            directory: base.join("mdglance").join("workspace-trust"),
        })
    }

    #[cfg(test)]
    fn at(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn is_trusted(&self, workspace: &Path) -> bool {
        let Ok(content) = fs::read_to_string(self.record_path(workspace)) else {
            return false;
        };
        let Ok(record) = toml::from_str::<TrustRecord>(&content) else {
            return false;
        };
        record.version == TRUST_VERSION && record.trusted && record.path == workspace
    }

    pub fn trust(&self, workspace: &Path) -> Result<()> {
        fs::create_dir_all(&self.directory)
            .with_context(|| format!("failed to create {}", self.directory.display()))?;

        let record = TrustRecord {
            version: TRUST_VERSION,
            path: workspace.to_path_buf(),
            trusted: true,
        };
        let content = toml::to_string_pretty(&record)?;
        let destination = self.record_path(workspace);
        let temporary = self.directory.join(format!(
            ".{}.{}.tmp",
            workspace_hash(workspace),
            std::process::id()
        ));
        fs::write(&temporary, content)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        fs::rename(&temporary, &destination)
            .with_context(|| format!("failed to save {}", destination.display()))?;
        Ok(())
    }

    fn record_path(&self, workspace: &Path) -> PathBuf {
        self.directory
            .join(format!("{}.toml", workspace_hash(workspace)))
    }
}

pub fn workspace_root(file: &Path) -> Result<PathBuf> {
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", file.display()))?;
    let parent = file
        .parent()
        .context("cannot determine the workspace of a file without a parent directory")?;

    for candidate in parent.ancestors() {
        if candidate.join(".git").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(parent.to_path_buf())
}

fn workspace_hash(workspace: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace.as_os_str().as_encoded_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_git_root_and_falls_back_to_parent() {
        let temp = std::env::temp_dir().join(format!("mdglance-trust-root-{}", std::process::id()));
        let repo = temp.join("repo");
        let nested = repo.join("docs");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("README.md");
        fs::write(&file, "test").unwrap();
        assert_eq!(workspace_root(&file).unwrap(), repo.canonicalize().unwrap());

        let loose = temp.join("loose.md");
        fs::write(&loose, "test").unwrap();
        assert_eq!(
            workspace_root(&loose).unwrap(),
            temp.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn validates_record_contents_as_well_as_filename() {
        let temp =
            std::env::temp_dir().join(format!("mdglance-trust-store-{}", std::process::id()));
        let store = TrustStore::at(temp.clone());
        let workspace = Path::new("/tmp/mdglance-workspace");
        assert!(!store.is_trusted(workspace));
        store.trust(workspace).unwrap();
        assert!(store.is_trusted(workspace));

        fs::write(
            store.record_path(workspace),
            "version = 1\npath = '/other'\ntrusted = true\n",
        )
        .unwrap();
        assert!(!store.is_trusted(workspace));
        let _ = fs::remove_dir_all(temp);
    }
}
