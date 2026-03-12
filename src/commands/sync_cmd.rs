use anyhow::Result;

use crate::api::client::MondayClient;
use crate::config::Config;
use crate::output;
use crate::sync::{pull, push, update};

pub async fn push(
    client: &MondayClient,
    cfg: &Config,
    beads_id: Option<&str>,
    epic: Option<&str>,
) -> Result<()> {
    if let Some(epic_id) = epic {
        let result = push::push_epic(client, cfg, epic_id).await?;
        output::success(&result)
    } else if let Some(id) = beads_id {
        let result = push::push_single(client, cfg, id).await?;
        output::success(&result)
    } else {
        let result = push::push_all(client, cfg).await?;
        output::success(&result)
    }
}

pub async fn pull(
    client: &MondayClient,
    cfg: &Config,
    monday_item_id: Option<&str>,
    parent: Option<&str>,
) -> Result<()> {
    if let Some(pid) = parent {
        let result = pull::pull_parent(client, cfg, pid).await?;
        output::success(&result)
    } else if let Some(id) = monday_item_id {
        let result = pull::pull_single(client, cfg, id).await?;
        output::success(&result)
    } else {
        let result = pull::pull_all(client, cfg).await?;
        output::success(&result)
    }
}

pub async fn full_sync(client: &MondayClient, cfg: &Config) -> Result<()> {
    let pull_result = pull::pull_all(client, cfg).await?;
    let push_result = push::push_all(client, cfg).await?;
    let combined = serde_json::json!({
        "pull": pull_result,
        "push": push_result,
    });
    output::success(&combined)
}

pub async fn update(
    client: &MondayClient,
    cfg: &Config,
    direction: &str,
    interactive: bool,
) -> Result<()> {
    let result = update::update_linked(client, cfg, direction, interactive).await?;
    output::success(&result)
}

pub async fn status(client: &MondayClient, cfg: &Config) -> Result<()> {
    let result = update::status(client, cfg).await?;
    output::success(&result)
}
