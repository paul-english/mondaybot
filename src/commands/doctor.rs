use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use std::path::Path;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{BoardsResponse, MeResponse};
use crate::beads::BeadsCli;
use crate::config::{self, Config};
use crate::output;
use crate::sync::mapping::SyncMapping;

#[derive(Serialize)]
struct Check {
    name: String,
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

impl Check {
    fn new(name: &str, status: &str, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            message: message.into(),
            hint: None,
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let mut checks: Vec<Check> = vec![];
    let mut any_fail = false;

    // 1. bd CLI available
    let bd = BeadsCli::from_cwd();
    match bd.check_available() {
        Ok(version) => checks.push(Check::new("bd_cli", "pass", version)),
        Err(_) => checks.push(
            Check::new("bd_cli", "warn", "not found")
                .with_hint("Install from https://github.com/steveyegge/beads"),
        ),
    }

    // 2. Beads initialized
    if bd.beads_dir_exists() {
        checks.push(Check::new("beads_init", "pass", ".beads/ found"));
    } else {
        checks.push(
            Check::new("beads_init", "warn", ".beads/ not found in cwd")
                .with_hint("Run `bd init` to initialize"),
        );
    }

    // 3. Config file present
    let cfg_path = config_path
        .map(|p| p.to_path_buf())
        .or_else(|| Config::config_path().ok());
    let cfg_exists = cfg_path.as_ref().is_some_and(|p| p.exists());
    if cfg_exists {
        checks.push(Check::new(
            "config_file",
            "pass",
            cfg_path.as_ref().unwrap().display().to_string(),
        ));
    } else {
        any_fail = true;
        checks.push(
            Check::new("config_file", "fail", "not found")
                .with_hint("Run `mondaybot config init`"),
        );
    }

    // 4. API token set
    let cfg = Config::load(config_path).unwrap_or_default();
    if cfg.api_token.is_empty() {
        any_fail = true;
        checks.push(
            Check::new("api_token", "fail", "not set").with_hint(format!(
                "Get a token at {} then: mondaybot config set api_token <token>",
                config::TOKEN_URL
            )),
        );
    } else {
        let source = if std::env::var("MONDAY_API_TOKEN").is_ok() {
            "from env"
        } else {
            "from config"
        };
        checks.push(Check::new("api_token", "pass", format!("set ({source})")));
    }

    // 5. monday.com API reachable
    if !cfg.api_token.is_empty() {
        let client = MondayClient::new(cfg.api_token.clone());
        match client.query::<MeResponse>(queries::ME, json!({})).await {
            Ok(me) => checks.push(Check::new(
                "api_reachable",
                "pass",
                format!("connected as {}", me.me.name),
            )),
            Err(e) => {
                any_fail = true;
                checks.push(
                    Check::new("api_reachable", "fail", format!("{e}")).with_hint(format!(
                        "Verify your token at {}",
                        config::TOKEN_URL
                    )),
                );
            }
        }

        // 6. Board ID configured and valid
        if let Some(board_id) = cfg.board_id {
            match client
                .query::<BoardsResponse>(queries::GET_BOARD, json!({ "boardId": [board_id] }))
                .await
            {
                Ok(resp) => {
                    if let Some(board) = resp.boards.first() {
                        checks.push(Check::new(
                            "board_id",
                            "pass",
                            format!(
                                "Board '{}' ({} columns, {} groups)",
                                board.name,
                                board.columns.len(),
                                board.groups.len()
                            ),
                        ));

                    } else {
                        checks.push(
                            Check::new("board_id", "warn", format!("board {board_id} not found"))
                                .with_hint("Run `mondaybot boards list` to see available boards"),
                        );
                    }
                }
                Err(e) => checks.push(Check::new(
                    "board_id",
                    "warn",
                    format!("failed to query board: {e}"),
                )),
            }
        } else {
            checks.push(
                Check::new("board_id", "warn", "not configured").with_hint(
                    "Run `mondaybot boards list` to find your board ID, then: mondaybot config set board_id <id>",
                ),
            );
        }
    } else {
        checks.push(Check::new("api_reachable", "fail", "skipped (no API token)"));
        checks.push(Check::new("board_id", "fail", "skipped (no API token)"));
    }

    // 8. Mapping file
    let mapping_path = SyncMapping::default_path();
    if mapping_path.exists() {
        match SyncMapping::load(&mapping_path) {
            Ok(m) => {
                checks.push(Check::new(
                    "mapping_file",
                    "info",
                    format!("{} linked items", m.entries.len()),
                ));
                if !m.entries.is_empty() && cfg.status_column.is_none() {
                    checks.push(
                        Check::new(
                            "status_column",
                            "warn",
                            "not set; status will not sync to monday.com",
                        )
                        .with_hint("Set the status column by name or id: mondaybot config set status_column \"Status\""),
                    );
                }
            }
            Err(e) => checks.push(Check::new(
                "mapping_file",
                "warn",
                format!("exists but invalid: {e}"),
            )),
        }
    } else {
        checks.push(Check::new(
            "mapping_file",
            "info",
            "no links yet (.beads/monday_sync.json not found)",
        ));
    }

    let pass_count = checks.iter().filter(|c| c.status == "pass").count();
    let warn_count = checks.iter().filter(|c| c.status == "warn").count();
    let fail_count = checks.iter().filter(|c| c.status == "fail").count();
    let summary = format!("{pass_count} passed, {warn_count} warnings, {fail_count} failed");

    let envelope = json!({
        "checks": checks,
        "summary": summary,
    });

    if any_fail {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": false,
                "data": envelope,
            }))?
        );
        Ok(())
    } else {
        output::success(&envelope)
    }
}
