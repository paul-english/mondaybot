use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{Item, SingleItemResponse};
use crate::beads::BeadsCli;
use crate::config::Config;
use crate::sync::mapping::{MappingEntry, SyncMapping};

#[derive(Serialize)]
pub struct UpdateResult {
    pub updated: Vec<UpdateAction>,
    pub conflicts: Vec<ConflictAction>,
    pub unchanged: Vec<String>,
    pub errors: Vec<ErrorAction>,
}

#[derive(Serialize)]
pub struct UpdateAction {
    pub beads_id: String,
    pub monday_item_id: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub direction: String,
}

#[derive(Serialize)]
pub struct ConflictAction {
    pub beads_id: String,
    pub monday_item_id: String,
    pub field: String,
    pub beads_value: String,
    pub monday_value: String,
    pub resolved: String,
}

#[derive(Serialize)]
pub struct ErrorAction {
    pub beads_id: String,
    pub error: String,
}

fn monday_label_to_beads_status(cfg: &Config, label: &str) -> String {
    for (beads_status, monday_label) in &cfg.status_map {
        if monday_label.eq_ignore_ascii_case(label) {
            return beads_status.clone();
        }
    }
    label.to_lowercase().replace(' ', "_")
}

fn column_text(item: &Item, col_id: &str) -> Option<String> {
    item.column_values
        .iter()
        .find(|cv| cv.id == col_id)
        .and_then(|cv| cv.text.clone())
        .filter(|t| !t.is_empty())
}

/// Refresh all already-linked items. Never creates anything.
pub async fn update_linked(
    client: &MondayClient,
    cfg: &Config,
    direction: &str,
) -> Result<UpdateResult> {
    let bd = BeadsCli::from_cwd();
    let mut mapping = SyncMapping::load_default()?;

    let mut result = UpdateResult {
        updated: vec![],
        conflicts: vec![],
        unchanged: vec![],
        errors: vec![],
    };

    let entries: Vec<MappingEntry> = mapping.entries.clone();

    for entry in &entries {
        let beads_result = bd.show(&entry.beads_id);
        let monday_result: Result<SingleItemResponse> = client
            .query(
                queries::GET_ITEM,
                json!({ "itemId": [&entry.monday_item_id] }),
            )
            .await;

        let beads_issue = match beads_result {
            Ok(issue) => issue,
            Err(e) => {
                result.errors.push(ErrorAction {
                    beads_id: entry.beads_id.clone(),
                    error: format!("beads issue not found: {e}"),
                });
                continue;
            }
        };

        let monday_item = match monday_result {
            Ok(resp) => match resp.items.into_iter().next() {
                Some(item) => item,
                None => {
                    result.errors.push(ErrorAction {
                        beads_id: entry.beads_id.clone(),
                        error: format!(
                            "monday item {} not found (deleted?)",
                            entry.monday_item_id
                        ),
                    });
                    continue;
                }
            },
            Err(e) => {
                result.errors.push(ErrorAction {
                    beads_id: entry.beads_id.clone(),
                    error: format!("monday API error: {e}"),
                });
                continue;
            }
        };

        // Compare status
        let monday_status = cfg
            .status_column
            .as_ref()
            .and_then(|col| column_text(&monday_item, col))
            .unwrap_or_default();
        let monday_beads_status = if monday_status.is_empty() {
            String::new()
        } else {
            monday_label_to_beads_status(cfg, &monday_status)
        };
        let beads_status = beads_issue.status.clone();

        let status_label = cfg
            .status_map
            .get(&beads_status)
            .cloned()
            .unwrap_or_else(|| beads_status.clone());

        let statuses_differ = !monday_beads_status.is_empty() && monday_beads_status != beads_status;
        let names_differ = monday_item.name != beads_issue.title;

        if !statuses_differ && !names_differ {
            result.unchanged.push(entry.beads_id.clone());
            continue;
        }

        let do_push = direction == "push" || direction == "both";
        let do_pull = direction == "pull" || direction == "both";

        // Status sync
        if statuses_differ {
            if do_push && do_pull {
                // Both sides differ — conflict. Beads wins.
                let board_id = cfg.require_board_id()?;
                if let Some(status_col) = &cfg.status_column {
                    let col_vals = json!({ status_col: { "label": status_label } }).to_string();
                    let vars = json!({
                        "boardId": board_id,
                        "itemId": &entry.monday_item_id,
                        "columnValues": col_vals,
                    });
                    let _ = client
                        .query::<serde_json::Value>(queries::UPDATE_ITEM, vars)
                        .await;
                }
                result.conflicts.push(ConflictAction {
                    beads_id: entry.beads_id.clone(),
                    monday_item_id: entry.monday_item_id.clone(),
                    field: "status".into(),
                    beads_value: beads_status.clone(),
                    monday_value: monday_beads_status.clone(),
                    resolved: "beads_wins".into(),
                });
            } else if do_push {
                let board_id = cfg.require_board_id()?;
                if let Some(status_col) = &cfg.status_column {
                    let col_vals = json!({ status_col: { "label": status_label } }).to_string();
                    let vars = json!({
                        "boardId": board_id,
                        "itemId": &entry.monday_item_id,
                        "columnValues": col_vals,
                    });
                    let _ = client
                        .query::<serde_json::Value>(queries::UPDATE_ITEM, vars)
                        .await;
                }
                result.updated.push(UpdateAction {
                    beads_id: entry.beads_id.clone(),
                    monday_item_id: entry.monday_item_id.clone(),
                    field: "status".into(),
                    old_value: monday_beads_status.clone(),
                    new_value: beads_status.clone(),
                    direction: "push".into(),
                });
            } else if do_pull {
                if monday_beads_status == "closed" {
                    let _ = bd.close(&entry.beads_id, Some("synced from monday.com"));
                } else {
                    let _ = bd.update_status(&entry.beads_id, &monday_beads_status);
                }
                result.updated.push(UpdateAction {
                    beads_id: entry.beads_id.clone(),
                    monday_item_id: entry.monday_item_id.clone(),
                    field: "status".into(),
                    old_value: beads_status.clone(),
                    new_value: monday_beads_status.clone(),
                    direction: "pull".into(),
                });
            }
        }

        // Update last_synced timestamp
        let now = chrono::Utc::now().to_rfc3339();
        mapping.add(MappingEntry {
            beads_id: entry.beads_id.clone(),
            monday_item_id: entry.monday_item_id.clone(),
            is_subitem: entry.is_subitem,
            parent_monday_id: entry.parent_monday_id.clone(),
            last_synced: now,
        });
    }

    mapping.save_default()?;
    Ok(result)
}

/// Show sync status without making any changes.
pub async fn status(client: &MondayClient, cfg: &Config) -> Result<serde_json::Value> {
    let bd = BeadsCli::from_cwd();
    let mapping = SyncMapping::load_default()?;

    let mut in_sync = 0u64;
    let mut drifted = vec![];
    let mut errors = vec![];

    for entry in &mapping.entries {
        let beads_result = bd.show(&entry.beads_id);
        let monday_result: Result<SingleItemResponse> = client
            .query(
                queries::GET_ITEM,
                json!({ "itemId": [&entry.monday_item_id] }),
            )
            .await;

        let beads_issue = match beads_result {
            Ok(i) => i,
            Err(e) => {
                errors.push(json!({
                    "beads_id": entry.beads_id,
                    "error": format!("beads issue not found: {e}")
                }));
                continue;
            }
        };

        let monday_item = match monday_result {
            Ok(resp) => match resp.items.into_iter().next() {
                Some(item) => item,
                None => {
                    errors.push(json!({
                        "beads_id": entry.beads_id,
                        "error": format!("monday item {} not found (deleted?)", entry.monday_item_id)
                    }));
                    continue;
                }
            },
            Err(e) => {
                errors.push(json!({
                    "beads_id": entry.beads_id,
                    "error": format!("monday API error: {e}")
                }));
                continue;
            }
        };

        let monday_status = cfg
            .status_column
            .as_ref()
            .and_then(|col| column_text(&monday_item, col))
            .unwrap_or_default();
        let monday_beads_status = if monday_status.is_empty() {
            String::new()
        } else {
            monday_label_to_beads_status(cfg, &monday_status)
        };

        if monday_beads_status == beads_issue.status || monday_beads_status.is_empty() {
            in_sync += 1;
        } else {
            // Figure out which side is newer based on updated_at vs last_synced
            let entry_obj = json!({
                "beads_id": entry.beads_id,
                "monday_item_id": entry.monday_item_id,
                "field": "status",
                "beads_value": beads_issue.status,
                "monday_value": monday_beads_status,
            });
            // Heuristic: we can't easily tell which side changed since last_synced
            // without monday timestamps, so report both as drifted
            drifted.push(entry_obj);
        }
    }

    Ok(json!({
        "total_linked": mapping.entries.len(),
        "in_sync": in_sync,
        "drifted": drifted,
        "errors": errors,
    }))
}
