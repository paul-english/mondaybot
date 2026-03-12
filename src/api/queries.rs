pub const LIST_BOARDS: &str = r#"
{
  boards(limit: 50) {
    id
    name
    columns { id title type }
    groups  { id title }
  }
}
"#;

pub const GET_BOARD: &str = r#"
query($boardId: [ID!]!) {
  boards(ids: $boardId) {
    id
    name
    columns { id title type }
    groups  { id title }
  }
}
"#;

/// Board with column settings for resolving status labels to indices.
pub const GET_BOARD_COLUMN_SETTINGS: &str = r#"
query($boardId: [ID!]!) {
  boards(ids: $boardId) {
    id
    columns { id title type settings_str }
  }
}
"#;

pub const LIST_ITEMS: &str = r#"
query($boardId: [ID!]!, $cursor: String) {
  boards(ids: $boardId) {
    items_page(limit: 100, cursor: $cursor) {
      cursor
      items {
        id
        name
        group { id title }
        column_values { id text value }
        subitems { id name column_values { id text value } }
      }
    }
  }
}
"#;

pub const GET_ITEM: &str = r#"
query($itemId: [ID!]!) {
  items(ids: $itemId) {
    id
    name
    column_values { id text value }
    subitems { id name column_values { id text value } }
  }
}
"#;

/// Fetch item with its board id (required for updating sub-items: they use their own board_id).
pub const GET_ITEM_BOARD: &str = r#"
query($itemId: [ID!]!) {
  items(ids: $itemId) {
    id
    board { id }
  }
}
"#;

pub const CREATE_ITEM: &str = r#"
mutation($boardId: ID!, $groupId: String, $itemName: String!, $columnValues: JSON!) {
  create_item(board_id: $boardId, group_id: $groupId, item_name: $itemName, column_values: $columnValues) {
    id
    name
  }
}
"#;

pub const UPDATE_ITEM: &str = r#"
mutation($boardId: ID!, $itemId: ID!, $columnValues: JSON!) {
  change_multiple_column_values(board_id: $boardId, item_id: $itemId, column_values: $columnValues) {
    id
    name
  }
}
"#;

pub const CREATE_SUBITEM: &str = r#"
mutation($parentId: ID!, $itemName: String!, $columnValues: JSON!) {
  create_subitem(parent_item_id: $parentId, item_name: $itemName, column_values: $columnValues) {
    id
    name
  }
}
"#;

pub const ME: &str = r#"
{
  me { id name }
}
"#;
