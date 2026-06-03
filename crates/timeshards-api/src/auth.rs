use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use axum::http::HeaderMap;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use timeshards_core::{
    permissions::{Action, PermissionSet, Resource},
    ApiError, AuthSession, CoreError,
};
use uuid::Uuid;

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn authenticate(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> anyhow::Result<Option<Uuid>> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE username = ? AND status = 'active'",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    let Some((user_id, hash)) = row else {
        return Ok(None);
    };

    let parsed = PasswordHash::new(&hash).map_err(|e| anyhow::anyhow!("invalid password hash: {e}"))?;
    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_err()
    {
        return Ok(None);
    }

    Ok(Some(Uuid::parse_str(&user_id)?))
}

/// Remove expired rows so the sessions table does not grow without bound.
pub async fn prune_expired_sessions(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_session(pool: &SqlitePool, user_id: Uuid) -> anyhow::Result<(String, String)> {
    let token = format!("ts_{}", Uuid::new_v4());
    let token_hash = hash_token(&token);
    let session_id = Uuid::new_v4();
    let expires_at = (Utc::now() + chrono::Duration::hours(12)).to_rfc3339();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(session_id.to_string())
    .bind(user_id.to_string())
    .bind(&token_hash)
    .bind(&expires_at)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok((token, expires_at))
}

pub async fn resolve_session(pool: &SqlitePool, token: &str) -> anyhow::Result<Option<AuthSession>> {
    let token_hash = hash_token(token);
    let row: Option<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT s.user_id, s.expires_at, u.username
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash = ?
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    let Some((user_id, expires_at, username)) = row else {
        return Ok(None);
    };

    if expires_at < Utc::now().to_rfc3339() {
        return Ok(None);
    }

    let user_id = Uuid::parse_str(&user_id)?;
    let display_name: String = sqlx::query_scalar("SELECT display_name FROM users WHERE id = ?")
        .bind(user_id.to_string())
        .fetch_one(pool)
        .await?;

    let locale: String = sqlx::query_scalar("SELECT locale FROM users WHERE id = ?")
        .bind(user_id.to_string())
        .fetch_one(pool)
        .await?;

    let roles: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT r.name, r.permissions_json
        FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = ?
        "#,
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;

    let mut role_names = Vec::new();
    let mut keys = Vec::new();
    for (name, json) in roles {
        role_names.push(name);
        if let Ok(k) = serde_json::from_str::<Vec<String>>(&json) {
            keys.extend(k);
        }
    }

    Ok(Some(AuthSession {
        session_id: Uuid::new_v4(),
        user_id,
        username,
        display_name,
        locale,
        roles: role_names,
        permissions: PermissionSet::from_keys(keys),
        expires_at: chrono::DateTime::parse_from_rfc3339(&expires_at)?.with_timezone(&Utc),
    }))
}

pub async fn auth_from_headers(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<AuthSession, ApiError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Anmeldung erforderlich"))?;

    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("Ungültiges Authorization-Format"))?;

    resolve_session(pool, token)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("Ungültige oder abgelaufene Sitzung"))
}

pub fn require_permission(
    session: &AuthSession,
    resource: Resource,
    action: Action,
) -> Result<(), ApiError> {
    if session.permissions.allows(resource, action) {
        Ok(())
    } else {
        Err(ApiError::forbidden("Keine Berechtigung für diese Aktion"))
    }
}

pub fn require_permission_core(
    session: &AuthSession,
    resource: Resource,
    action: Action,
) -> Result<(), CoreError> {
    if session.permissions.allows(resource, action) {
        Ok(())
    } else {
        Err(CoreError::Forbidden("Keine Berechtigung".into()))
    }
}

/// HR/manager roles that may act on other employees' time and absence data.
pub fn can_manage_others(session: &AuthSession) -> bool {
    session.roles.iter().any(|r| {
        matches!(r.as_str(), "system_admin" | "hr_admin" | "manager")
    })
}

/// Roles that may list or export access events for all employees (not only self).
pub fn can_view_all_access_events(session: &AuthSession) -> bool {
    can_manage_others(session)
        || session
            .roles
            .iter()
            .any(|r| r == "security_operator")
}
