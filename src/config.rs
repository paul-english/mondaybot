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
}

fn default_status_map() -> HashMap<String, String> {
    HashMap::from([
        ("open".into(), "Open".into()),
        ("in_progress".into(), "Working on it".into()),
        ("closed".into(), "Done".into()),
    ])
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_token: String::new(),
            board_id: None,
            status_column: None,
            status_map: default_status_map(),
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
        match key {
            "api_token" => self.api_token = value.to_string(),
            "board_id" => {
                self.board_id = Some(value.parse().context("board_id must be a number")?);
            }
            "status_column" => self.status_column = Some(value.to_string()),
            _ => bail!("unknown config key: {key}. Valid keys: api_token, board_id, status_column"),
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
