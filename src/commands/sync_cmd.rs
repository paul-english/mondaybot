use anyhow::{Result, bail};

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
        bail!("specify a <beads-id> or --epic <beads-id> to push")
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
        bail!("specify a <monday-item-id> or --parent <monday-item-id> to pull")
    }
}

pub async fn update(
    client: &MondayClient,
    cfg: &Config,
    direction: &str,
) -> Result<()> {
    let result = update::update_linked(client, cfg, direction).await?;
    output::success(&result)
}

pub async fn status(client: &MondayClient, cfg: &Config) -> Result<()> {
    let result = update::status(client, cfg).await?;
    output::success(&result)
}
