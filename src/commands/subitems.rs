use anyhow::Result;
use serde_json::json;

use crate::api::client::MondayClient;
use crate::api::queries;
use crate::api::types::{CreateSubitemResponse, SingleItemResponse};
use crate::output;

pub async fn list(client: &MondayClient, parent_id: &str) -> Result<()> {
    let resp: SingleItemResponse = client
        .query(queries::GET_ITEM, json!({ "itemId": [parent_id] }))
        .await?;
    match resp.items.into_iter().next() {
        Some(item) => {
            let subs = item.subitems.unwrap_or_default();
            output::success(&subs)
        }
        None => output::error_json(&format!("parent item {parent_id} not found")),
    }
}

pub async fn create(
    client: &MondayClient,
    parent_id: &str,
    name: &str,
    column_values: Option<&str>,
) -> Result<()> {
    let vars = json!({
        "parentId": parent_id,
        "itemName": name,
        "columnValues": column_values.unwrap_or("{}"),
    });
    let resp: CreateSubitemResponse = client.query(queries::CREATE_SUBITEM, vars).await?;
    output::success(&resp.create_subitem)
}
