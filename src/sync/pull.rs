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
pub struct PullResult {
    pub created: Vec<PullAction>,
    pub updated: Vec<PullAction>,
}

#[derive(Serialize)]
pub struct PullAction {
    pub beads_id: String,
    pub monday_item_id: String,
    pub name: String,
}

/// Reverse-map a monday status label to a beads status string.
fn monday_label_to_beads_status(cfg: &Config, label: &str) -> String {
    for (beads_status, monday_label) in &cfg.status_map {
        if monday_label.eq_ignore_ascii_case(label) {
            return beads_status.clone();
        }
    }
    label.to_lowercase().replace(' ', "_")
}

/// Extract the text value for a given column ID from a monday item.
fn column_text(item: &Item, col_id: &str) -> Option<String> {
    item.column_values
        .iter()
        .find(|cv| cv.id == col_id)
        .and_then(|cv| cv.text.clone())
        .filter(|t| !t.is_empty())
}

/// Pull one monday item into beads. Creates a beads issue if not linked, updates if linked.
async fn pull_one(
    _client: &MondayClient,
    cfg: &Config,
    bd: &BeadsCli,
    mapping: &mut SyncMapping,
    item: &Item,
    issue_type: &str,
    parent_beads_id: Option<&str>,
) -> Result<PullAction> {
    let now = chrono::Utc::now().to_rfc3339();

    // Check if already linked
    if let Some(existing) = mapping.find_by_monday_id(&item.id) {
        let beads_id = existing.beads_id.clone();

        // Try to update beads issue status from monday
        if let Some(status_col) = &cfg.status_column {
            if let Some(label) = column_text(item, status_col) {
                let beads_status = monday_label_to_beads_status(cfg, &label);
                if beads_status == "closed" {
                    let _ = bd.close(&beads_id, Some("synced from monday.com"));
                } else {
                    let _ = bd.update_status(&beads_id, &beads_status);
                }
            }
        }

        // Update last_synced
        mapping.add(MappingEntry {
            beads_id: beads_id.clone(),
            monday_item_id: item.id.clone(),
            is_subitem: existing.is_subitem,
            parent_monday_id: existing.parent_monday_id.clone(),
            last_synced: now,
        });

        return Ok(PullAction {
            beads_id,
            monday_item_id: item.id.clone(),
            name: item.name.clone(),
        });
    }

    // Create a new beads issue
    let new_issue = bd.create(&item.name, issue_type, 2, parent_beads_id)?;

    mapping.add(MappingEntry {
        beads_id: new_issue.id.clone(),
        monday_item_id: item.id.clone(),
        is_subitem: parent_beads_id.is_some(),
        parent_monday_id: None,
        last_synced: now,
    });

    Ok(PullAction {
        beads_id: new_issue.id,
        monday_item_id: item.id.clone(),
        name: item.name.clone(),
    })
}

/// Pull a single monday item by ID.
pub async fn pull_single(
    client: &MondayClient,
    cfg: &Config,
    monday_item_id: &str,
) -> Result<PullResult> {
    let bd = BeadsCli::from_cwd();

    let resp: SingleItemResponse = client
        .query(queries::GET_ITEM, json!({ "itemId": [monday_item_id] }))
        .await?;
    let item = resp
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("monday item {monday_item_id} not found"))?;

    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let was_linked = mapping.find_by_monday_id(monday_item_id).is_some();
    let action = pull_one(client, cfg, &bd, &mut mapping, &item, "task", None).await?;

    mapping.save_default()?;

    let mut result = PullResult {
        created: vec![],
        updated: vec![],
    };
    if was_linked {
        result.updated.push(action);
    } else {
        result.created.push(action);
    }
    Ok(result)
}

/// Pull a parent item and all its sub-items.
pub async fn pull_parent(
    client: &MondayClient,
    cfg: &Config,
    monday_item_id: &str,
) -> Result<PullResult> {
    let bd = BeadsCli::from_cwd();

    let resp: SingleItemResponse = client
        .query(queries::GET_ITEM, json!({ "itemId": [monday_item_id] }))
        .await?;
    let item = resp
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("monday item {monday_item_id} not found"))?;

    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let mut result = PullResult {
        created: vec![],
        updated: vec![],
    };

    let was_linked = mapping.find_by_monday_id(monday_item_id).is_some();
    let parent_action =
        pull_one(client, cfg, &bd, &mut mapping, &item, "epic", None).await?;
    let parent_beads_id = parent_action.beads_id.clone();
    if was_linked {
        result.updated.push(parent_action);
    } else {
        result.created.push(parent_action);
    }

    if let Some(subitems) = &item.subitems {
        for sub in subitems {
            let was_linked = mapping.find_by_monday_id(&sub.id).is_some();
            let action = pull_one(
                client,
                cfg,
                &bd,
                &mut mapping,
                sub,
                "task",
                Some(&parent_beads_id),
            )
            .await?;
            if was_linked {
                result.updated.push(action);
            } else {
                result.created.push(action);
            }
        }
    }

    mapping.save_default()?;
    Ok(result)
}
