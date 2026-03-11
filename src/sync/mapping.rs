use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAPPING_FILENAME: &str = "monday_sync.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMapping {
    pub board_id: u64,
    #[serde(default)]
    pub entries: Vec<MappingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingEntry {
    pub beads_id: String,
    pub monday_item_id: String,
    #[serde(default)]
    pub is_subitem: bool,
    pub parent_monday_id: Option<String>,
    pub last_synced: String,
}

impl SyncMapping {
    pub fn mapping_path(beads_dir: &Path) -> PathBuf {
        beads_dir.join(MAPPING_FILENAME)
    }

    /// Default path: .beads/monday_sync.json relative to cwd.
    pub fn default_path() -> PathBuf {
        PathBuf::from(".beads").join(MAPPING_FILENAME)
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                board_id: 0,
                entries: vec![],
            });
        }
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("invalid JSON in {}", path.display()))
    }

    pub fn load_default() -> Result<Self> {
        Self::load(&Self::default_path())
    }

    /// Atomic write: write to a tmp file then rename.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, &json)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    pub fn save_default(&self) -> Result<()> {
        self.save(&Self::default_path())
    }

    pub fn find_by_beads_id(&self, id: &str) -> Option<&MappingEntry> {
        self.entries.iter().find(|e| e.beads_id == id)
    }

    pub fn find_by_monday_id(&self, id: &str) -> Option<&MappingEntry> {
        self.entries.iter().find(|e| e.monday_item_id == id)
    }

    pub fn add(&mut self, entry: MappingEntry) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.beads_id == entry.beads_id)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub fn remove_by_beads_id(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.beads_id != id);
        self.entries.len() < before
    }
}
