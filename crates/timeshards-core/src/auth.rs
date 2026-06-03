use crate::permissions::PermissionSet;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub locale: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_no: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub locale: String,
    pub roles: Vec<String>,
    pub permissions: PermissionSet,
    pub expires_at: DateTime<Utc>,
}

impl AuthSession {
    pub fn to_summary(&self) -> UserSummary {
        UserSummary {
            id: self.user_id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            locale: self.locale.clone(),
            roles: self.roles.clone(),
            permissions: self.permissions.keys().cloned().collect(),
            employee_id: None,
            employee_no: None,
        }
    }
}
