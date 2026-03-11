use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub group: Option<Group>,
    #[serde(default)]
    pub column_values: Vec<ColumnValue>,
    pub subitems: Option<Vec<Item>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnValue {
    pub id: String,
    pub text: Option<String>,
    pub value: Option<String>,
}

// --- Response wrappers ---

#[derive(Debug, Deserialize)]
pub struct BoardsResponse {
    pub boards: Vec<Board>,
}

#[derive(Debug, Deserialize)]
pub struct ItemsPageResponse {
    pub boards: Vec<BoardWithItemsPage>,
}

#[derive(Debug, Deserialize)]
pub struct BoardWithItemsPage {
    pub items_page: ItemsPage,
}

#[derive(Debug, Deserialize)]
pub struct ItemsPage {
    pub cursor: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
pub struct SingleItemResponse {
    pub items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
pub struct CreateItemResponse {
    pub create_item: Item,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubitemResponse {
    pub create_subitem: Item,
}

#[derive(Debug, Deserialize)]
pub struct ChangeColumnValuesResponse {
    pub change_multiple_column_values: Item,
}

#[derive(Debug, Deserialize)]
pub struct MeResponse {
    pub me: MeUser,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MeUser {
    pub id: serde_json::Value,
    pub name: String,
}
