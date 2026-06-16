use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    ClaudeCode,
    QwenCode,
    QoderCli,
    CursorCli,
    KimiCli,
    Codex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number,
    Boolean,
    Select,
    Combobox,
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFieldSchema {
    pub key: String,
    pub label: String,
    pub field_type: FieldType,
    pub required: bool,
    pub options: Vec<String>,
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFormSchema {
    pub title: String,
    pub fields: Vec<ToolFieldSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCatalogItem {
    pub kind: ToolKind,
    pub display_name: String,
    pub schema: ToolFormSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInstance {
    pub id: String,
    pub kind: ToolKind,
    pub name: String,
    pub enabled: bool,
    pub version: u64,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolIndexFile {
    pub instances: BTreeMap<String, ToolInstance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateToolInstanceRequest {
    pub kind: ToolKind,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateToolInstanceRequest {
    pub name: String,
    pub enabled: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchToolInstanceEnabledRequest {
    pub enabled: bool,
}
