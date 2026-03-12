# mondaybot

A Rust CLI that provides opt-in, bidirectional sync between
[beads](https://github.com/steveyegge/beads) (a local, git-tracked issue
tracker) and [monday.com](https://monday.com) boards.

Only **explicitly linked** issues are ever synced. Running `mondaybot sync update`
is always safe — it refreshes linked items but never creates anything new.

All output is JSON, wrapped in `{"ok": true, "data": ...}` /
`{"ok": false, "error": "..."}` envelopes, so it works equally well for
humans, scripts, and AI agents.

## Prerequisites

| Dependency | Why |
|---|---|
| **Rust 1.85+** | The crate uses `edition = "2024"` |
| **bd** ([beads CLI](https://github.com/steveyegge/beads)) | Local issue tracker that mondaybot syncs with monday.com |
| **monday.com API token** | Obtain from <https://arupinnovation.monday.com/apps/manage/tokens> |

You also need a `.beads/` directory in your project (`bd init`).

## Installation

```bash
# from a clone of this repo
cargo install --path .

# or build without installing
cargo build --release
# binary at target/release/mondaybot
```

## Quick start

```bash
# 1. Create the config file
mondaybot config init

# 2. Add your API token
mondaybot config set api_token <YOUR_TOKEN>

# 3. Find your board
mondaybot boards list

# 4. Set the default board
mondaybot config set board_id <BOARD_ID>

# 5. Verify everything works
mondaybot doctor
```

## Workflow

A typical PM-to-developer handoff:

1. PM creates a task in monday.com (or you create one with `mondaybot items create`).
2. Create a matching beads epic: `bd create "Epic title" -t epic`.
3. Link them: `mondaybot link add <beads-id> <monday-item-id>`.
4. Work on sub-tasks in beads: `bd create "Sub-task" --parent <epic-id>`.
5. Push sub-tasks that matter: `mondaybot sync push <sub-task-beads-id>`.
6. Keep linked items fresh: `mondaybot sync update`.
7. Check drift: `mondaybot sync status`.

Only push issues to monday.com that are relevant to project tracking — most
beads issues are internal dev tasks that don't need to appear on the board.

## Configuration

Config lives at `<platform-config-dir>/mondaybot/config.json`
(e.g. `~/.config/mondaybot/config.json` on Linux/macOS).

```json
{
  "api_token": "...",
  "board_id": 1234567890,
  "status_column": "Status",
  "status_map": {
    "open": "To Do",
    "in_progress": "In Progress",
    "closed": "Done",
    "cancelled": "Blocked"
  }
}
```

Beads statuses (**open**, **in_progress**, **closed**, **cancelled**) are mapped via `status_map` to your board’s status **labels** (e.g. "To Do", "In Progress", "Done"). The tool looks up each label in the board’s status column settings and sends the **integer index** to the API. Values in `status_map` should match the label text shown in monday.com (case-insensitive). Override via CLI: `mondaybot config set status_map.in_progress "Started"`.

| Key | Description |
|---|---|
| `api_token` | monday.com API token |
| `board_id` | Default board ID for item operations |
| `status_column` | **Column name or ID** for status sync (e.g. `"Status"`). Required for status to sync. |
| `status_map` | Maps beads status → monday.com label. Beads statuses include: `open`, `in_progress`, `closed`, `cancelled` (and any custom labels your workflow uses). Set with `config set status_map.<beads_status> "Monday Label"`. |
| `name_column` | **Column name or ID** for item title (default: `"name"`). Used when pushing title changes. |
| `owner_column` | **Column name or ID** of a People column. When set, newly created items/subitems are assigned to the current user. |

Column fields (`status_column`, `name_column`, `owner_column`) accept either the board column’s **title** (e.g. `"Status"`, `"Name"`) or its API column ID; the tool resolves names to IDs using the board’s columns.

### Environment variable overrides

| Variable | Overrides |
|---|---|
| `MONDAY_API_TOKEN` | `api_token` |
| `MONDAY_BOARD_ID` | `board_id` |

### Global CLI flags

Every subcommand accepts these optional flags:

- `--config <path>` — use a different config file
- `--token <token>` — override the API token for this invocation
- `--board-id <id>` — override the board ID for this invocation

## Commands

### config

```bash
mondaybot config init                        # Create default config file
mondaybot config show                        # Print current config (token masked)
mondaybot config set <key> <value>           # Set a config value
```

### doctor

Run diagnostic health checks — verifies the `bd` CLI, `.beads/` directory,
config file, API token, API connectivity, board validity, and mapping file.

```bash
mondaybot doctor
```

### setup

Write AI-agent integration instructions into a project so that coding assistants
know how to use mondaybot.

```bash
mondaybot setup agents [--dir <path>]        # Append/update section in AGENTS.md
mondaybot setup cursor [--dir <path>]        # Write .cursor/rules/mondaybot.mdc
```

Both commands are idempotent — they use markers to replace existing sections on
re-run.

### boards

```bash
mondaybot boards list                        # List all accessible boards
mondaybot boards get [--board-id <id>]       # Show columns and groups for a board
```

### items

```bash
mondaybot items list [--cursor <token>]      # Paginated item listing
mondaybot items get --item-id <id>           # Get item with sub-items
mondaybot items create --name "Title" \
  [--group-id <gid>] \
  [--column-values '<json>']                 # Create a new item
mondaybot items update --item-id <id> \
  --column-values '<json>'                   # Update column values
```

### subitems

```bash
mondaybot subitems list --parent-id <id>     # List sub-items of a parent
mondaybot subitems create --parent-id <id> \
  --name "Title" \
  [--column-values '<json>']                 # Create a sub-item
mondaybot subitems update --item-id <id> \
  --column-values '<json>'                   # Update a sub-item
```

### link

Manage the opt-in registry that ties beads issues to monday.com items.

```bash
mondaybot link add <beads-id> <monday-id>    # Link a beads issue to a monday item
mondaybot link remove <beads-id>             # Unlink a beads issue
mondaybot link list                          # Show all current links
```

### sync

```bash
mondaybot sync sync                          # Full sync: pull (with discovery) then push (with discovery)
mondaybot sync push                          # Push all linked items
mondaybot sync push <beads-id>               # Push one issue (creates if unlinked)
mondaybot sync push --epic <beads-id>        # Push an epic and all child tasks
mondaybot sync pull                          # Pull all linked items (discovers new monday sub-items)
mondaybot sync pull <monday-id>              # Pull one item (creates beads issue if unlinked)
mondaybot sync pull --parent <monday-id>     # Pull parent + all sub-items
mondaybot sync update [--direction <dir>] [--interactive]  # Refresh linked only, no creates
mondaybot sync status                        # Show what's linked and what's drifted
```

- **`sync sync`** — Full bidirectional sync: runs pull (which discovers new monday sub-items under linked parents and creates them in beads), then push (which pushes linked items and beads children of linked epics). Use this to sync new sub-items created on either side.
- **`--direction`** (for update) accepts `push`, `pull`, or `both` (default: `both`).
- **`--interactive`** / **`-i`** (for update) — When status differs on both sides, prompt on stderr/stdin to choose (b)eads, (m)onday, or (s)kip instead of auto-resolving with beads wins.

## How sync works

### Mapping file

Links are stored in `.beads/monday_sync.json` inside your project. This file
is intended to be committed to git so the whole team shares the same link
registry.

```json
{
  "board_id": 1234567890,
  "entries": [
    {
      "beads_id": "abc-1",
      "monday_item_id": "9876543210",
      "is_subitem": false,
      "parent_monday_id": null,
      "last_synced": "2026-03-11T12:00:00Z"
    }
  ]
}
```

### Push (beads -> monday.com)

- If the beads issue is already linked, only its **status** is updated on monday.com
  (and only when `status_column` is set in config; otherwise the update is a no-op).
- If unlinked, a new monday.com item (or sub-item, if there's a linked parent
  epic) is created and the link is recorded.
- **`sync push` with no args** pushes all linked items, then for each linked **epic**
  also pushes its beads children that aren't linked yet, creating them as monday
  sub-items and adding them to the mapping.
- `--epic` pushes the epic itself plus all child tasks in one operation.

### Pull (monday.com -> beads)

- If the monday item is already linked, the beads issue status is updated.
- If unlinked, a new beads issue is created via `bd create` and the link is
  recorded.
- **`sync pull` with no args** also discovers new monday sub-items under linked
  parents: for each linked item, fetches its sub-items from monday and pulls any
  not yet in the mapping into beads (as tasks under the parent epic), then adds
  them to the mapping.
- `--parent` pulls the parent item as an epic and all its sub-items as tasks.

### Update

`sync update` iterates over every entry in the mapping file and compares
statuses. It **never creates** items on either side.

- `--direction push` — beads status wins, pushes to monday.
- `--direction pull` — monday status wins, pulls into beads.
- `--direction both` (default) — on conflict, **beads wins** (unless `--interactive`).
- `--interactive` / `-i` — When both sides have different status, prompt to choose (b)eads, (m)onday, or (s)kip.

### Status

`sync status` is read-only. It reports how many linked items are in sync,
which ones have drifted, and any errors (e.g. deleted items on either side).

## AI agent integration

mondaybot can inject usage instructions into your project so that AI coding
assistants (Cursor, Codex, etc.) know how to interact with it:

```bash
mondaybot setup agents          # writes/updates AGENTS.md
mondaybot setup cursor          # writes .cursor/rules/mondaybot.mdc
```

Both are idempotent and use begin/end markers to replace their section on
subsequent runs.

## License

See repository for license details.
