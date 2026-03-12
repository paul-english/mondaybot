use serde::{Deserialize, Deserializer, Serialize};

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

/// Column with settings (e.g. status labels and indices).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnWithSettings {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub settings_str: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardWithColumnSettings {
    pub id: String,
    #[serde(default)]
    pub columns: Vec<ColumnWithSettings>,
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
    pub board: Option<BoardRef>,
}

/// Board id from API may be string or number.
fn deserialize_id_optional<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;
    struct IdVisitor;
    impl<'de> Visitor<'de> for IdVisitor {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "id as string or number")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }
    }
    d.deserialize_any(IdVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardRef {
    #[serde(deserialize_with = "deserialize_id_optional")]
    pub id: String,
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
pub struct BoardsWithSettingsResponse {
    pub boards: Vec<BoardWithColumnSettings>,
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

/// Minimal item shape for GET_ITEM_BOARD (id + board only).
#[derive(Debug, Deserialize)]
pub struct ItemBoardInfo {
    #[serde(deserialize_with = "deserialize_id_optional")]
    pub id: String,
    pub board: Option<BoardRef>,
}

#[derive(Debug, Deserialize)]
pub struct ItemBoardResponse {
    pub items: Vec<ItemBoardInfo>,
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
