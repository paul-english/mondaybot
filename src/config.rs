use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const TOKEN_URL: &str = "https://arupinnovation.monday.com/apps/manage/tokens";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api_token: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    pub status_column: Option<String>,
    #[serde(default = "default_status_map")]
    pub status_map: HashMap<String, String>,
    pub name_column: Option<String>,
    pub owner_column: Option<String>,
}

fn default_status_map() -> HashMap<String, String> {
    // Labels must match the board's status column exactly (e.g. "To Do", "In Progress", "Done")
    HashMap::from([
        ("open".into(), "To Do".into()),
        ("in_progress".into(), "In Progress".into()),
        ("closed".into(), "Done".into()),
        ("cancelled".into(), "Blocked".into()),
    ])
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_token: String::new(),
            board_id: None,
            status_column: None,
            status_map: default_status_map(),
            name_column: None,
            owner_column: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("could not determine config directory")?
            .join("mondaybot");
        Ok(dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Load config from file, then overlay environment variables.
    pub fn load(path_override: Option<&Path>) -> Result<Self> {
        let path = match path_override {
            Some(p) => p.to_path_buf(),
            None => Self::config_path()?,
        };

        let mut config = if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config from {}", path.display()))?;
            serde_json::from_str(&contents)
                .with_context(|| format!("invalid JSON in {}", path.display()))?
        } else {
            Config::default()
        };

        if let Ok(token) = std::env::var("MONDAY_API_TOKEN") {
            config.api_token = token;
        }
        if let Ok(board) = std::env::var("MONDAY_BOARD_ID") {
            if let Ok(id) = board.parse::<u64>() {
                config.board_id = Some(id);
            }
        }

        Ok(config)
    }

    /// Load config, failing with a helpful message if the API token is missing.
    pub fn load_or_fail(path_override: Option<&Path>) -> Result<Self> {
        let config = Self::load(path_override)?;
        if config.api_token.is_empty() {
            bail!(
                "No API token configured. Get one at {TOKEN_URL} then:\n  \
                 export MONDAY_API_TOKEN=<token>\n  or: mondaybot config set api_token <token>"
            );
        }
        Ok(config)
    }

    pub fn save(&self, path_override: Option<&Path>) -> Result<()> {
        let path = match path_override {
            Some(p) => p.to_path_buf(),
            None => Self::config_path()?,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(beads_status) = key.strip_prefix("status_map.") {
            if beads_status.is_empty() {
                bail!("status_map.<beads_status> requires a beads status, e.g. status_map.in_progress");
            }
            self.status_map
                .insert(beads_status.to_string(), value.to_string());
            return Ok(());
        }
        match key {
            "api_token" => self.api_token = value.to_string(),
            "board_id" => {
                self.board_id = Some(value.parse().context("board_id must be a number")?);
            }
            "status_column" => self.status_column = Some(value.to_string()),
            "name_column" => self.name_column = Some(value.to_string()),
            "owner_column" => self.owner_column = Some(value.to_string()),
            _ => bail!(
                "unknown config key: {key}. Valid keys: api_token, board_id, status_column, name_column, owner_column, status_map.<beads_status> (e.g. status_map.in_progress)"
            ),
        }
        Ok(())
    }

    pub fn require_board_id(&self) -> Result<u64> {
        self.board_id.context(
            "No board_id configured. Run `mondaybot boards list` to find your board ID, then:\n  \
             mondaybot config set board_id <id>",
        )
    }
}
