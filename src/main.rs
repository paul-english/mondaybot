mod api;
mod beads;
mod commands;
mod config;
mod output;
mod sync;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mondaybot", about = "CLI tool for monday.com task management")]
struct Cli {
    /// Path to config file override
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Override API token
    #[arg(long, global = true)]
    token: Option<String>,

    /// Override board ID
    #[arg(long, global = true)]
    board_id: Option<u64>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Run diagnostic health checks
    Doctor,
    /// Write agent integration instructions to a repo
    Setup {
        #[command(subcommand)]
        target: SetupTarget,
    },
    /// Board operations
    Boards {
        #[command(subcommand)]
        action: BoardsAction,
    },
    /// Item (top-level task) operations
    Items {
        #[command(subcommand)]
        action: ItemsAction,
    },
    /// Sub-item operations
    Subitems {
        #[command(subcommand)]
        action: SubitemsAction,
    },
    /// Manage beads <-> monday.com links (opt-in registry)
    Link {
        #[command(subcommand)]
        action: LinkAction,
    },
    /// Beads <-> monday.com sync (operates ONLY on linked items)
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Initialize config interactively
    Init,
    /// Show current config
    Show,
    /// Set a config value
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum SetupTarget {
    /// Append mondaybot section to AGENTS.md
    Agents {
        /// Target directory (defaults to cwd)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Write .cursor/rules/mondaybot.mdc
    Cursor {
        /// Target directory (defaults to cwd)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BoardsAction {
    /// List all accessible boards
    List,
    /// Get board details (columns, groups)
    Get {
        #[arg(long)]
        board_id: Option<u64>,
    },
}

#[derive(Subcommand)]
enum ItemsAction {
    /// List items on a board
    List {
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Get item by ID (includes sub-items)
    Get {
        #[arg(long)]
        item_id: String,
    },
    /// Create a new item
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        group_id: Option<String>,
        #[arg(long)]
        column_values: Option<String>,
    },
    /// Update an item
    Update {
        #[arg(long)]
        item_id: String,
        #[arg(long)]
        column_values: String,
    },
}

#[derive(Subcommand)]
enum SubitemsAction {
    /// List sub-items of a parent
    List {
        #[arg(long)]
        parent_id: String,
    },
    /// Create a sub-item
    Create {
        #[arg(long)]
        parent_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        column_values: Option<String>,
    },
    /// Update a sub-item (same as item update)
    Update {
        #[arg(long)]
        item_id: String,
        #[arg(long)]
        column_values: String,
    },
}

#[derive(Subcommand)]
enum LinkAction {
    /// Link a beads issue to a monday item
    Add {
        beads_id: String,
        monday_item_id: String,
    },
    /// Unlink a beads issue
    Remove { beads_id: String },
    /// Show all current links
    List,
}

#[derive(Subcommand)]
enum SyncAction {
    /// Push beads issue(s) to monday.com
    Push {
        /// Beads issue ID to push
        beads_id: Option<String>,
        /// Push an epic and all its child tasks
        #[arg(long)]
        epic: Option<String>,
    },
    /// Pull monday.com item(s) into beads
    Pull {
        /// Monday item ID to pull
        monday_item_id: Option<String>,
        /// Pull a parent item and all its sub-items
        #[arg(long)]
        parent: Option<String>,
    },
    /// Full sync: pull (with discovery) then push (with discovery)
    Sync {
        /// Prompt to resolve conflicts (for future use)
        #[arg(short, long)]
        interactive: bool,
    },
    /// Refresh all already-linked items (never creates anything)
    Update {
        #[arg(long, default_value = "both")]
        direction: String,
        #[arg(short, long)]
        interactive: bool,
    },
    /// Show sync status: what's linked, what's drifted
    Status,
}

fn build_client(cli: &Cli, cfg: &config::Config) -> api::client::MondayClient {
    let token = cli
        .token
        .clone()
        .unwrap_or_else(|| cfg.api_token.clone());
    api::client::MondayClient::new(token)
}

fn effective_board_id(cli: &Cli, cfg: &config::Config) -> Result<u64> {
    if let Some(id) = cli.board_id {
        return Ok(id);
    }
    cfg.require_board_id()
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config_path = cli.config.as_deref();

    let result: Result<()> = (async {
        match &cli.command {
        Commands::Config { action } => match action {
            ConfigAction::Init => {
                let cfg = config::Config::default();
                cfg.save(config_path)?;
                let path = config_path
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| config::Config::config_path().unwrap());
                output::success(&serde_json::json!({
                    "action": "created",
                    "path": path.display().to_string(),
                    "message": format!("Config created. Get your API token at {} then: mondaybot config set api_token <YOUR_TOKEN>", config::TOKEN_URL)
                }))
            }
            ConfigAction::Show => {
                let cfg = config::Config::load(config_path)?;
                let mut display = serde_json::to_value(&cfg)?;
                if let Some(obj) = display.as_object_mut() {
                    if obj.get("api_token").and_then(|v| v.as_str()).map_or(false, |s| !s.is_empty()) {
                        obj.insert("api_token".into(), serde_json::json!("***"));
                    }
                }
                output::success(&display)
            }
            ConfigAction::Set { key, value } => {
                let mut cfg = config::Config::load(config_path)?;
                cfg.set_value(key, value)?;
                cfg.save(config_path)?;
                output::success(&serde_json::json!({ "set": key, "value": value }))
            }
        },

        Commands::Doctor => commands::doctor::run(config_path).await,

        Commands::Setup { target } => match target {
            SetupTarget::Agents { dir } => {
                let d = dir.clone().unwrap_or_else(|| PathBuf::from("."));
                commands::setup::write_agents(&d)
            }
            SetupTarget::Cursor { dir } => {
                let d = dir.clone().unwrap_or_else(|| PathBuf::from("."));
                commands::setup::write_cursor(&d)
            }
        },

        Commands::Boards { action } => {
            let cfg = config::Config::load_or_fail(config_path)?;
            let client = build_client(&cli, &cfg);
            match action {
                BoardsAction::List => commands::boards::list(&client).await,
                BoardsAction::Get { board_id } => {
                    let id = board_id.unwrap_or(effective_board_id(&cli, &cfg)?);
                    commands::boards::get(&client, id).await
                }
            }
        }

        Commands::Items { action } => {
            let cfg = config::Config::load_or_fail(config_path)?;
            let client = build_client(&cli, &cfg);
            match action {
                ItemsAction::List { cursor } => {
                    let bid = effective_board_id(&cli, &cfg)?;
                    commands::items::list(&client, bid, cursor.as_deref()).await
                }
                ItemsAction::Get { item_id } => {
                    commands::items::get(&client, item_id).await
                }
                ItemsAction::Create {
                    name,
                    group_id,
                    column_values,
                } => {
                    let bid = effective_board_id(&cli, &cfg)?;
                    commands::items::create(
                        &client,
                        bid,
                        name,
                        group_id.as_deref(),
                        column_values.as_deref(),
                    )
                    .await
                }
                ItemsAction::Update {
                    item_id,
                    column_values,
                } => {
                    let bid = effective_board_id(&cli, &cfg)?;
                    commands::items::update(&client, bid, item_id, column_values).await
                }
            }
        }

        Commands::Subitems { action } => {
            let cfg = config::Config::load_or_fail(config_path)?;
            let client = build_client(&cli, &cfg);
            match action {
                SubitemsAction::List { parent_id } => {
                    commands::subitems::list(&client, parent_id).await
                }
                SubitemsAction::Create {
                    parent_id,
                    name,
                    column_values,
                } => {
                    commands::subitems::create(&client, parent_id, name, column_values.as_deref())
                        .await
                }
                SubitemsAction::Update {
                    item_id,
                    column_values,
                } => {
                    let bid = effective_board_id(&cli, &cfg)?;
                    commands::items::update(&client, bid, item_id, column_values).await
                }
            }
        }

        Commands::Link { action } => {
            let cfg = config::Config::load_or_fail(config_path)?;
            let client = build_client(&cli, &cfg);
            match action {
                LinkAction::Add {
                    beads_id,
                    monday_item_id,
                } => commands::link::add(&client, &cfg, beads_id, monday_item_id).await,
                LinkAction::Remove { beads_id } => commands::link::remove(beads_id),
                LinkAction::List => commands::link::list(),
            }
        }

        Commands::Sync { action } => {
            let cfg = config::Config::load_or_fail(config_path)?;
            let client = build_client(&cli, &cfg);
            match action {
                SyncAction::Push { beads_id, epic } => {
                    commands::sync_cmd::push(&client, &cfg, beads_id.as_deref(), epic.as_deref())
                        .await
                }
                SyncAction::Pull {
                    monday_item_id,
                    parent,
                } => {
                    commands::sync_cmd::pull(
                        &client,
                        &cfg,
                        monday_item_id.as_deref(),
                        parent.as_deref(),
                    )
                    .await
                }
                SyncAction::Sync { interactive: _ } => {
                    commands::sync_cmd::full_sync(&client, &cfg).await
                }
                SyncAction::Update { direction, interactive } => {
                    commands::sync_cmd::update(&client, &cfg, direction, *interactive).await
                }
                SyncAction::Status => commands::sync_cmd::status(&client, &cfg).await,
            }
        }
    }
    })
    .await;

    if let Err(e) = result {
        let _ = output::error_json(&format!("{e:#}"));
        std::process::exit(1);
    }
}
