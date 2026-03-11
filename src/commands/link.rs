use anyhow::Result;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::SingleItemResponse;
use crate::beads::BeadsCli;
use crate::config::Config;
use crate::output;
use crate::sync::mapping::{MappingEntry, SyncMapping};

pub async fn add(
    client: &MondayClient,
    cfg: &Config,
    beads_id: &str,
    monday_item_id: &str,
) -> Result<()> {
    let bd = BeadsCli::from_cwd();

    let issue = bd.show(beads_id)?;

    let resp: SingleItemResponse = client
        .query(queries::GET_ITEM, json!({ "itemId": [monday_item_id] }))
        .await?;
    let monday_item = resp
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("monday item {monday_item_id} not found"))?;

    let is_subitem = false;

    let mut mapping = SyncMapping::load_default()?;
    if mapping.board_id == 0 {
        mapping.board_id = cfg.board_id.unwrap_or(0);
    }

    let now = chrono::Utc::now().to_rfc3339();
    mapping.add(MappingEntry {
        beads_id: beads_id.to_string(),
        monday_item_id: monday_item_id.to_string(),
        is_subitem,
        parent_monday_id: None,
        last_synced: now,
    });
    mapping.save_default()?;

    output::success(&json!({
        "linked": {
            "beads_id": beads_id,
            "beads_title": issue.title,
            "monday_item_id": monday_item_id,
            "monday_item_name": monday_item.name,
        }
    }))
}

pub fn remove(beads_id: &str) -> Result<()> {
    let mut mapping = SyncMapping::load_default()?;
    if mapping.remove_by_beads_id(beads_id) {
        mapping.save_default()?;
        output::success(&json!({ "unlinked": beads_id }))
    } else {
        output::error_json(&format!("no link found for beads id: {beads_id}"))
    }
}

pub fn list() -> Result<()> {
    let mapping = SyncMapping::load_default()?;
    output::success(&json!({
        "board_id": mapping.board_id,
        "links": mapping.entries,
    }))
}
