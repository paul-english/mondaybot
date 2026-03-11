use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{
    ChangeColumnValuesResponse, CreateItemResponse, Item, ItemsPageResponse, SingleItemResponse,
};
use crate::output;

#[derive(Serialize)]
struct ItemsListOutput {
    items: Vec<Item>,
    next_cursor: Option<String>,
}

pub async fn list(
    client: &MondayClient,
    board_id: u64,
    cursor: Option<&str>,
) -> Result<()> {
    let vars = json!({
        "boardId": [board_id],
        "cursor": cursor,
    });
    let resp: ItemsPageResponse = client.query(queries::LIST_ITEMS, vars).await?;
    let page = resp
        .boards
        .into_iter()
        .next()
        .map(|b| b.items_page)
        .unwrap_or_else(|| crate::api::types::ItemsPage {
            cursor: None,
            items: vec![],
        });

    output::success(&ItemsListOutput {
        items: page.items,
        next_cursor: page.cursor,
    })
}

pub async fn get(client: &MondayClient, item_id: &str) -> Result<()> {
    let resp: SingleItemResponse = client
        .query(
            queries::GET_ITEM,
            json!({ "itemId": [item_id] }),
        )
        .await?;
    match resp.items.into_iter().next() {
        Some(item) => output::success(&item),
        None => output::error_json(&format!("item {item_id} not found")),
    }
}

pub async fn create(
    client: &MondayClient,
    board_id: u64,
    name: &str,
    group_id: Option<&str>,
    column_values: Option<&str>,
) -> Result<()> {
    let vars = json!({
        "boardId": board_id,
        "itemName": name,
        "groupId": group_id,
        "columnValues": column_values.unwrap_or("{}"),
    });
    let resp: CreateItemResponse = client.query(queries::CREATE_ITEM, vars).await?;
    output::success(&resp.create_item)
}

pub async fn update(
    client: &MondayClient,
    board_id: u64,
    item_id: &str,
    column_values: &str,
) -> Result<()> {
    let vars = json!({
        "boardId": board_id,
        "itemId": item_id,
        "columnValues": column_values,
    });
    let resp: ChangeColumnValuesResponse = client.query(queries::UPDATE_ITEM, vars).await?;
    output::success(&resp.change_multiple_column_values)
}
