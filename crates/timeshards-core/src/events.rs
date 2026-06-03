use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventActor {
    User { id: Uuid },
    Service { id: String },
    Device { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    pub topic: String,
    pub schema_version: u32,
    pub occurred_at: DateTime<Utc>,
    pub producer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<EventActor>,
    pub payload: Value,
}

impl EventEnvelope {
    pub fn new(topic: impl Into<String>, producer: impl Into<String>, payload: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            schema_version: 1,
            occurred_at: Utc::now(),
            producer: producer.into(),
            correlation_id: None,
            actor: None,
            payload,
        }
    }
}

/// Well-known domain event topics.
pub mod topics {
    pub const TIME_CLOCK_IN: &str = "time.clock.in";
    pub const TIME_CLOCK_OUT: &str = "time.clock.out";
    pub const TIME_BREAK_START: &str = "time.break.start";
    pub const TIME_BREAK_END: &str = "time.break.end";
    pub const ACCESS_DECISION: &str = "access.decision.recorded";
    pub const BADGE_PRESENTED: &str = "access.badge.presented";
    pub const USER_LOGIN: &str = "identity.user.login";
}

pub type DomainEvent = EventEnvelope;
