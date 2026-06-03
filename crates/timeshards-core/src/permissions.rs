use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Canonical resources — extend when adding shards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    User,
    Employee,
    Role,
    Badge,
    Zone,
    Door,
    Schedule,
    Shift,
    TimeEvent,
    Timesheet,
    Absence,
    AccessRule,
    AccessEvent,
    Report,
    SystemConfig,
    AuditLog,
    HardwareDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Read,
    Create,
    Update,
    Delete,
    Approve,
    Override,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    pub resource: Resource,
    pub action: Action,
}

impl Permission {
    pub const fn new(resource: Resource, action: Action) -> Self {
        Self { resource, action }
    }

    pub fn key(&self) -> String {
        format!("{}:{}", serde_key(&self.resource), serde_key(&self.action))
    }
}

fn serde_key<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionSet {
    permissions: HashSet<String>,
}

impl PermissionSet {
    pub fn new<I: IntoIterator<Item = Permission>>(perms: I) -> Self {
        Self {
            permissions: perms.into_iter().map(|p| p.key()).collect(),
        }
    }

    pub fn from_keys<I: IntoIterator<Item = String>>(keys: I) -> Self {
        Self {
            permissions: keys.into_iter().collect(),
        }
    }

    pub fn allows(&self, resource: Resource, action: Action) -> bool {
        let key = Permission::new(resource, action).key();
        self.permissions.contains(&key) || self.permissions.contains(&format!("{key}:*"))
    }

    pub fn insert(&mut self, perm: Permission) {
        self.permissions.insert(perm.key());
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.permissions.iter()
    }
}

/// Built-in role templates (German labels in UI; keys stay English).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleTemplate {
    SystemAdmin,
    HrAdmin,
    SecurityOperator,
    Manager,
    Employee,
}

impl RoleTemplate {
    pub fn name_de(&self) -> &'static str {
        match self {
            Self::SystemAdmin => "Systemadministrator",
            Self::HrAdmin => "Personalverwaltung",
            Self::SecurityOperator => "Sicherheit",
            Self::Manager => "Vorgesetzte/r",
            Self::Employee => "Mitarbeiter/in",
        }
    }

    pub fn permissions(&self) -> PermissionSet {
        match self {
            Self::SystemAdmin => PermissionSet::new(all_admin()),
            Self::HrAdmin => PermissionSet::new([
                Permission::new(Resource::Employee, Action::Read),
                Permission::new(Resource::Employee, Action::Create),
                Permission::new(Resource::Employee, Action::Update),
                Permission::new(Resource::TimeEvent, Action::Read),
                Permission::new(Resource::Timesheet, Action::Read),
                Permission::new(Resource::Timesheet, Action::Approve),
                Permission::new(Resource::Shift, Action::Read),
                Permission::new(Resource::Shift, Action::Create),
                Permission::new(Resource::Shift, Action::Update),
                Permission::new(Resource::Absence, Action::Read),
                Permission::new(Resource::Absence, Action::Create),
                Permission::new(Resource::Absence, Action::Approve),
                Permission::new(Resource::TimeEvent, Action::Override),
                Permission::new(Resource::Report, Action::Read),
                Permission::new(Resource::Report, Action::Export),
            ]),
            Self::SecurityOperator => PermissionSet::new([
                Permission::new(Resource::Zone, Action::Read),
                Permission::new(Resource::Door, Action::Read),
                Permission::new(Resource::Door, Action::Update),
                Permission::new(Resource::Badge, Action::Read),
                Permission::new(Resource::Badge, Action::Create),
                Permission::new(Resource::Badge, Action::Update),
                Permission::new(Resource::AccessRule, Action::Read),
                Permission::new(Resource::AccessRule, Action::Create),
                Permission::new(Resource::AccessRule, Action::Update),
                Permission::new(Resource::AccessRule, Action::Delete),
                Permission::new(Resource::AccessEvent, Action::Read),
                Permission::new(Resource::HardwareDevice, Action::Read),
                Permission::new(Resource::AuditLog, Action::Read),
            ]),
            Self::Manager => PermissionSet::new([
                Permission::new(Resource::Employee, Action::Read),
                Permission::new(Resource::TimeEvent, Action::Read),
                Permission::new(Resource::TimeEvent, Action::Approve),
                Permission::new(Resource::Timesheet, Action::Read),
                Permission::new(Resource::Timesheet, Action::Approve),
                Permission::new(Resource::Shift, Action::Read),
                Permission::new(Resource::Absence, Action::Read),
                Permission::new(Resource::Absence, Action::Create),
                Permission::new(Resource::Absence, Action::Approve),
                Permission::new(Resource::AccessEvent, Action::Read),
                Permission::new(Resource::Report, Action::Read),
                Permission::new(Resource::Report, Action::Export),
            ]),
            Self::Employee => PermissionSet::new([
                Permission::new(Resource::TimeEvent, Action::Read),
                Permission::new(Resource::TimeEvent, Action::Create),
                Permission::new(Resource::Shift, Action::Read),
                Permission::new(Resource::Timesheet, Action::Read),
                Permission::new(Resource::Timesheet, Action::Update),
                Permission::new(Resource::Absence, Action::Read),
                Permission::new(Resource::Absence, Action::Create),
                Permission::new(Resource::Badge, Action::Read),
            ]),
        }
    }
}

fn all_admin() -> Vec<Permission> {
    use Action::*;
    use Resource::*;
    let resources = [
        User,
        Employee,
        Role,
        Badge,
        Zone,
        Door,
        Schedule,
        Shift,
        TimeEvent,
        Timesheet,
        Absence,
        AccessRule,
        AccessEvent,
        Report,
        SystemConfig,
        AuditLog,
        HardwareDevice,
    ];
    let actions = [Read, Create, Update, Delete, Approve, Override, Export];
    resources
        .into_iter()
        .flat_map(|r| actions.into_iter().map(move |a| Permission::new(r, a)))
        .collect()
}
