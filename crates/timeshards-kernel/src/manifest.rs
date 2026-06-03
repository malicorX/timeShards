use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub permissions: Vec<ShardPermission>,
    #[serde(default)]
    pub publishes: Vec<String>,
    #[serde(default)]
    pub subscribes: Vec<String>,
    #[serde(default)]
    pub ui: Option<ShardUiManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ShardPermission {
    pub key: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardUiManifest {
    #[serde(default)]
    pub widgets: Vec<WidgetContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetContribution {
    pub slot: UiSlot,
    pub component: String,
    pub title: String,
    #[serde(default)]
    pub min_role: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSlot {
    NavPrimary,
    DashboardCenter,
    DashboardRight,
    AdminPanel,
    TimeHome,
    AccessLive,
}
