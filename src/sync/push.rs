use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{CreateItemResponse, CreateSubitemResponse};
use crate::beads::{self, BeadsCli, BeadsIssue};
use crate::config::Config;
use crate::sync::mapping::{MappingEntry, SyncMapping};

#[derive(Serialize)]
pub struct PushResult {
    pub created: Vec<PushAction>,
    pub updated: Vec<PushAction>,
    pub skipped: Vec<PushAction>,
}

#[derive(Serialize)]
pub struct PushAction {
    pub beads_id: String,
    pub monday_item_id: String,
    pub title: String,
}

/// Push a single beads issue to monday.com. Creates if not yet linked, updates if linked.
pub async fn push_one(
    client: &MondayClient,
    cfg: &Config,
    mapping: &mut SyncMapping,
    issue: &BeadsIssue,
    parent_monday_id: Option<&str>,
) -> Result<PushAction> {
    let board_id = cfg.require_board_id()?;
    let now = chrono::Utc::now().to_rfc3339();

    let status_label = cfg
        .status_map
        .get(&issue.status)
        .cloned()
        .unwrap_or_else(|| issue.status.clone());

    if let Some(existing) = mapping.find_by_beads_id(&issue.id) {
        let monday_id = existing.monday_item_id.clone();
        let mut col_vals = json!({});
        if let Some(status_col) = &cfg.status_column {
            col_vals[status_col] = json!({ "label": status_label });
        }

        let vars = json!({
            "boardId": board_id,
            "itemId": monday_id,
            "columnValues": col_vals.to_string(),
        });
        let _ = client
            .query::<serde_json::Value>(queries::UPDATE_ITEM, vars)
            .await?;

        // Update last_synced
        mapping.add(MappingEntry {
            beads_id: issue.id.clone(),
            monday_item_id: monday_id.clone(),
            is_subitem: existing.is_subitem,
            parent_monday_id: existing.parent_monday_id.clone(),
            last_synced: now,
        });

        Ok(PushAction {
            beads_id: issue.id.clone(),
            monday_item_id: monday_id,
            title: issue.title.clone(),
        })
    } else {
        let mut col_vals = json!({});
        if let Some(status_col) = &cfg.status_column {
            col_vals[status_col] = json!({ "label": status_label });
        }

        let monday_id = if let Some(pid) = parent_monday_id {
            let vars = json!({
                "parentId": pid,
                "itemName": issue.title,
                "columnValues": col_vals.to_string(),
            });
            let resp: CreateSubitemResponse =
                client.query(queries::CREATE_SUBITEM, vars).await?;
            resp.create_subitem.id
        } else {
            let vars = json!({
                "boardId": board_id,
                "itemName": issue.title,
                "columnValues": col_vals.to_string(),
            });
            let resp: CreateItemResponse = client.query(queries::CREATE_ITEM, vars).await?;
            resp.create_item.id
        };

        mapping.add(MappingEntry {
            beads_id: issue.id.clone(),
            monday_item_id: monday_id.clone(),
            is_subitem: parent_monday_id.is_some(),
            parent_monday_id: parent_monday_id.map(String::from),
            last_synced: now,
        });

        Ok(PushAction {
            beads_id: issue.id.clone(),
            monday_item_id: monday_id,
            title: issue.title.clone(),
        })
    }
}

/// Push an epic and all its child tasks.
pub async fn push_epic(
    client: &MondayClient,
    cfg: &Config,
    epic_id: &str,
) -> Result<PushResult> {
    let bd = BeadsCli::from_cwd();

    let epic = bd.show(epic_id)?;
    if epic.issue_type != "epic" {
        bail!("{epic_id} is a {} not an epic", epic.issue_type);
    }

    let all_issues = bd.list(None)?;
    let children = beads::all_children(&all_issues, epic_id);

    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let mut result = PushResult {
        created: vec![],
        updated: vec![],
        skipped: vec![],
    };

    let was_linked = mapping.find_by_beads_id(epic_id).is_some();
    let action = push_one(client, cfg, &mut mapping, &epic, None).await?;
    if was_linked {
        result.updated.push(action);
    } else {
        result.created.push(action);
    }

    let epic_monday_id = mapping
        .find_by_beads_id(epic_id)
        .map(|e| e.monday_item_id.clone())
        .unwrap();

    for child in &children {
        let was_linked = mapping.find_by_beads_id(&child.id).is_some();
        let action =
            push_one(client, cfg, &mut mapping, child, Some(&epic_monday_id)).await?;
        if was_linked {
            result.updated.push(action);
        } else {
            result.created.push(action);
        }
    }

    mapping.save_default()?;
    Ok(result)
}

/// Push a single beads issue by ID.
pub async fn push_single(
    client: &MondayClient,
    cfg: &Config,
    beads_id: &str,
) -> Result<PushResult> {
    let bd = BeadsCli::from_cwd();
    let issue = bd.show(beads_id)?;

    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let was_linked = mapping.find_by_beads_id(beads_id).is_some();

    // If the issue has a parent epic that's already linked, push as sub-item
    let parent_monday_id = issue
        .dependencies
        .as_ref()
        .and_then(|deps| {
            deps.iter().find_map(|d| {
                mapping
                    .find_by_beads_id(&d.depends_on_id)
                    .map(|m| m.monday_item_id.clone())
            })
        });

    let action = push_one(
        client,
        cfg,
        &mut mapping,
        &issue,
        parent_monday_id.as_deref(),
    )
    .await?;

    mapping.save_default()?;

    let mut result = PushResult {
        created: vec![],
        updated: vec![],
        skipped: vec![],
    };
    if was_linked {
        result.updated.push(action);
    } else {
        result.created.push(action);
    }
    Ok(result)
}
