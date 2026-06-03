pub mod auth;
pub mod error;
pub mod events;
pub mod ids;
pub mod permissions;

pub use auth::{AuthSession, LoginRequest, LoginResponse, UserSummary};
pub use error::{ApiError, ApiResult, CoreError};
pub use events::{DomainEvent, EventActor, EventEnvelope};
pub use ids::{BadgeId, DoorId, EmployeeId, SessionId, SiteId, UserId, ZoneId};
pub use permissions::{Action, Permission, PermissionSet, Resource, RoleTemplate};
