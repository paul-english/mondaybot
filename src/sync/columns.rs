//! Resolve column by id or by title (name) so users can set status_column "Status" etc.
//! Resolve status labels to monday.com status column indices for API updates.

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{BoardsResponse, BoardsWithSettingsResponse};

/// Resolve a column reference (id or display name) to the column id for the given board.
/// If column_ref matches a column id exactly, returns it. Otherwise matches column title
/// case-insensitively. Returns None if not found or if column_ref is empty.
pub async fn resolve_column_id(
    client: &MondayClient,
    board_id: u64,
    column_ref: &str,
) -> Result<Option<String>> {
    let column_ref = column_ref.trim();
    if column_ref.is_empty() {
        return Ok(None);
    }

    let resp: BoardsResponse = client
        .query(queries::GET_BOARD, json!({ "boardId": [board_id] }))
        .await?;

    let board = match resp.boards.into_iter().next() {
        Some(b) => b,
        None => return Ok(None),
    };

    for col in &board.columns {
        if col.id == column_ref {
            return Ok(Some(col.id.clone()));
        }
        if col.title.eq_ignore_ascii_case(column_ref) {
            return Ok(Some(col.id.clone()));
        }
    }

    Ok(None)
}

/// Settings JSON for status columns: labels array with index and label.
#[derive(serde::Deserialize)]
struct StatusSettings {
    labels: Option<Vec<StatusLabel>>,
}

#[derive(serde::Deserialize)]
struct StatusLabel {
    #[serde(default)]
    index: Option<i64>,
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// Fetch the status column's label -> index map for the given board.
/// Returns a map from label text (case-insensitive key) to the integer index to send to the API.
pub async fn status_label_to_index(
    client: &MondayClient,
    board_id: u64,
    status_column_ref: &str,
) -> Result<HashMap<String, u64>> {
    let mut out = HashMap::new();

    let resp: BoardsWithSettingsResponse = client
        .query(queries::GET_BOARD_COLUMN_SETTINGS, json!({ "boardId": [board_id] }))
        .await?;

    let board = match resp.boards.into_iter().next() {
        Some(b) => b,
        None => return Ok(out),
    };

    let status_col_id = match resolve_column_id(client, board_id, status_column_ref).await? {
        Some(id) => id,
        None => return Ok(out),
    };

    let col = match board
        .columns
        .iter()
        .find(|c| c.id == status_col_id || c.title.eq_ignore_ascii_case(status_column_ref))
    {
        Some(c) => c,
        None => return Ok(out),
    };

    let settings_str = match &col.settings_str {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(out),
    };

    let settings: StatusSettings = serde_json::from_str(settings_str)
        .context("parse status column settings_str")?;

    if let Some(labels) = settings.labels {
        for entry in labels {
            let idx = entry.index.or(entry.id).unwrap_or(0);
            let label = entry
                .label
                .or(entry.text)
                .unwrap_or_default();
            if !label.is_empty() {
                out.insert(label, idx as u64);
            }
        }
    }

    Ok(out)
}

/// Find the status index for a label; tries exact match then case-insensitive.
pub fn status_index_for_label(label_map: &HashMap<String, u64>, label: &str) -> Option<u64> {
    if let Some(&idx) = label_map.get(label) {
        return Some(idx);
    }
    let label_lower = label.to_lowercase();
    for (k, &v) in label_map {
        if k.eq_ignore_ascii_case(&label_lower) || k.eq_ignore_ascii_case(label) {
            return Some(v);
        }
    }
    None
}
