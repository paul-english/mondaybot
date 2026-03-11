use anyhow::Result;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::BoardsResponse;
use crate::output;

pub async fn list(client: &MondayClient) -> Result<()> {
    let resp: BoardsResponse = client.query(queries::LIST_BOARDS, json!({})).await?;
    output::success(&resp.boards)
}

pub async fn get(client: &MondayClient, board_id: u64) -> Result<()> {
    let resp: BoardsResponse = client
        .query(queries::GET_BOARD, json!({ "boardId": [board_id] }))
        .await?;
    match resp.boards.into_iter().next() {
        Some(board) => output::success(&board),
        None => output::error_json(&format!("board {board_id} not found")),
    }
}
