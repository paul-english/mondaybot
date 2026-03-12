use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{
    CreateItemResponse, CreateSubitemResponse, ItemBoardResponse, MeResponse,
};
use crate::beads::{self, BeadsCli, BeadsIssue};
use crate::config::Config;
use crate::sync::columns;
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
/// When `existing_entry` is Some (e.g. from push_all), use it instead of looking up by issue.id,
/// so that mappings with short ids like "kjv" still match after bd returns canonical "task-kjv".
pub async fn push_one(
    client: &MondayClient,
    cfg: &Config,
    mapping: &mut SyncMapping,
    issue: &BeadsIssue,
    parent_monday_id: Option<&str>,
    existing_entry: Option<&MappingEntry>,
) -> Result<PushAction> {
    let board_id = cfg.require_board_id()?;
    let now = chrono::Utc::now().to_rfc3339();

    let status_label = cfg
        .status_map
        .get(&issue.status)
        .cloned()
        .unwrap_or_else(|| issue.status.clone());

    let existing = existing_entry.or_else(|| mapping.find_by_beads_id(&issue.id));
    if let Some(existing) = existing {
        let monday_id = existing.monday_item_id.clone();

        let update_board_id = if existing.is_subitem {
            let resp: ItemBoardResponse = client
                .query(queries::GET_ITEM_BOARD, json!({ "itemId": [&monday_id] }))
                .await?;
            resp.items
                .into_iter()
                .next()
                .and_then(|i| i.board)
                .and_then(|b| b.id.parse().ok())
                .unwrap_or(board_id)
        } else {
            board_id
        };

        let name_ref = cfg.name_column.as_deref().unwrap_or("name");
        let name_col = columns::resolve_column_id(client, update_board_id, name_ref)
            .await?
            .unwrap_or_else(|| name_ref.to_string());
        let mut col_vals = json!({});
        col_vals[name_col] = serde_json::json!(issue.title.trim());
        if let Some(status_ref) = &cfg.status_column {
            if let Some(status_col) =
                columns::resolve_column_id(client, update_board_id, status_ref).await?
            {
                if let Ok(label_to_index) =
                    columns::status_label_to_index(client, update_board_id, status_ref).await
                {
                    if let Some(idx) = columns::status_index_for_label(&label_to_index, &status_label)
                    {
                        col_vals[status_col] = json!({ "index": idx });
                    }
                }
            }
        }

        let vars = json!({
            "boardId": update_board_id,
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
        if let Some(status_ref) = &cfg.status_column {
            if let Some(status_col) =
                columns::resolve_column_id(client, board_id, status_ref).await?
            {
                if let Ok(label_to_index) =
                    columns::status_label_to_index(client, board_id, status_ref).await
                {
                    if let Some(idx) = columns::status_index_for_label(&label_to_index, &status_label)
                    {
                        col_vals[status_col] = json!({ "index": idx });
                    }
                }
            }
        }
        if let Some(owner_ref) = &cfg.owner_column {
            if let Some(owner_col) =
                columns::resolve_column_id(client, board_id, owner_ref).await?
            {
                if let Ok(me_resp) = client
                    .query::<MeResponse>(queries::ME, json!({}))
                    .await
                {
                    let user_id = me_resp.me.id.as_u64()
                        .or_else(|| me_resp.me.id.as_i64().map(|n| n as u64))
                        .or_else(|| me_resp.me.id.as_str().and_then(|s| s.parse().ok()));
                    if let Some(id) = user_id {
                        col_vals[owner_col] = json!({
                            "personsAndTeams": [{ "id": id, "kind": "person" }]
                        });
                    }
                }
            }
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
    let action = push_one(client, cfg, &mut mapping, &epic, None, None).await?;
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
            push_one(client, cfg, &mut mapping, child, Some(&epic_monday_id), None).await?;
        if was_linked {
            result.updated.push(action);
        } else {
            result.created.push(action);
        }
    }

    mapping.save_default()?;
    Ok(result)
}

/// Push all linked items. For any linked epic, also pushes its beads children that
/// are not yet linked, creating them on monday as sub-items.
pub async fn push_all(
    client: &MondayClient,
    cfg: &Config,
) -> Result<PushResult> {
    let bd = BeadsCli::from_cwd();
    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let mut result = PushResult {
        created: vec![],
        updated: vec![],
        skipped: vec![],
    };

    let entries: Vec<MappingEntry> = mapping.entries.clone();
    for entry in &entries {
        match bd.show(&entry.beads_id) {
            Ok(issue) => {
                let parent = entry.parent_monday_id.clone();
                match push_one(
                    client,
                    cfg,
                    &mut mapping,
                    &issue,
                    parent.as_deref(),
                    Some(entry),
                )
                .await
                {
                    Ok(action) => result.updated.push(action),
                    Err(e) => result.skipped.push(PushAction {
                        beads_id: entry.beads_id.clone(),
                        monday_item_id: entry.monday_item_id.clone(),
                        title: format!("error: {e}"),
                    }),
                }
            }
            Err(e) => result.skipped.push(PushAction {
                beads_id: entry.beads_id.clone(),
                monday_item_id: entry.monday_item_id.clone(),
                title: format!("beads error: {e}"),
            }),
        }
    }

    // For each linked epic, push its beads children that aren't in the mapping yet.
    let all_issues = bd.list(None).unwrap_or_default();
    for entry in &entries {
        let issue = match bd.show(&entry.beads_id) {
            Ok(i) => i,
            Err(_) => continue,
        };
        if issue.issue_type != "epic" {
            continue;
        }
        let mut children = beads::all_children(&all_issues, &entry.beads_id);
        for candidate in &all_issues {
            if candidate.dependency_count == 0 {
                continue;
            }
            if children.iter().any(|c| c.id == candidate.id) {
                continue;
            }
            let full = match bd.show(&candidate.id) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let depends_on_epic = full.dependencies.as_ref().map_or(false, |deps| {
                deps.iter().any(|d| beads::dependency_refs_epic(d, &entry.beads_id))
            });
            if depends_on_epic {
                children.push(full);
            }
        }
        for child in &children {
            if mapping.find_by_beads_id(&child.id).is_some() {
                continue;
            }
            match push_one(
                client,
                cfg,
                &mut mapping,
                child,
                Some(&entry.monday_item_id),
                None,
            )
            .await
            {
                Ok(action) => result.created.push(action),
                Err(e) => result.skipped.push(PushAction {
                    beads_id: child.id.clone(),
                    monday_item_id: entry.monday_item_id.clone(),
                    title: format!("error: {e}"),
                }),
            }
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
                let parent_id = d.id.as_deref().or(d.depends_on_id.as_deref())?;
                mapping
                    .find_by_beads_id(parent_id)
                    .map(|m| m.monday_item_id.clone())
            })
        });

    let action = push_one(
        client,
        cfg,
        &mut mapping,
        &issue,
        parent_monday_id.as_deref(),
        None,
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
