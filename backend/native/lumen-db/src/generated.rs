//! Row structs generated from the SQLAlchemy models. Do not edit.
//!
//! Regenerate with:
//!
//! ```text
//! uv run python backend/native/lumen-db/scripts/generate_models.py
//! ```
//!
//! `backend/tests/unit/lumen/db/test_generated_models.py` fails when this
//! file drifts from `backend/lumen/db/models.py`.

#![allow(clippy::struct_field_names)]

use serde::{Deserialize, Serialize};

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionApprovalDecidedVia {
    #[serde(rename = "USER")]
    User,
    #[serde(rename = "PRE_APPROVAL")]
    PreApproval,
    #[serde(rename = "SESSION_GRANT")]
    SessionGrant,
}

impl ActionApprovalDecidedVia {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::PreApproval => "PRE_APPROVAL",
            Self::SessionGrant => "SESSION_GRANT",
        }
    }
}

impl std::str::FromStr for ActionApprovalDecidedVia {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "USER" => Ok(Self::User),
            "PRE_APPROVAL" => Ok(Self::PreApproval),
            "SESSION_GRANT" => Ok(Self::SessionGrant),
            other => Err(crate::UnknownEnumValue {
                enum_name: "ActionApprovalDecidedVia",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionApprovalDecision {
    #[serde(rename = "APPROVED")]
    Approved,
    #[serde(rename = "REJECTED")]
    Rejected,
    #[serde(rename = "EXPIRED")]
    Expired,
}

impl ActionApprovalDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Expired => "EXPIRED",
        }
    }
}

impl std::str::FromStr for ActionApprovalDecision {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "APPROVED" => Ok(Self::Approved),
            "REJECTED" => Ok(Self::Rejected),
            "EXPIRED" => Ok(Self::Expired),
            other => Err(crate::UnknownEnumValue {
                enum_name: "ActionApprovalDecision",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildSessionOrigin {
    #[serde(rename = "INTERACTIVE")]
    Interactive,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "SLACK")]
    Slack,
}

impl BuildSessionOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "INTERACTIVE",
            Self::Scheduled => "SCHEDULED",
            Self::Slack => "SLACK",
        }
    }
}

impl std::str::FromStr for BuildSessionOrigin {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "INTERACTIVE" => Ok(Self::Interactive),
            "SCHEDULED" => Ok(Self::Scheduled),
            "SLACK" => Ok(Self::Slack),
            other => Err(crate::UnknownEnumValue {
                enum_name: "BuildSessionOrigin",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildSessionStatus {
    #[serde(rename = "INITIALIZING")]
    Initializing,
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "IDLE")]
    Idle,
    #[serde(rename = "FAILED")]
    Failed,
}

impl BuildSessionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "INITIALIZING",
            Self::Active => "ACTIVE",
            Self::Idle => "IDLE",
            Self::Failed => "FAILED",
        }
    }
}

impl std::str::FromStr for BuildSessionStatus {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "INITIALIZING" => Ok(Self::Initializing),
            "ACTIVE" => Ok(Self::Active),
            "IDLE" => Ok(Self::Idle),
            "FAILED" => Ok(Self::Failed),
            other => Err(crate::UnknownEnumValue {
                enum_name: "BuildSessionStatus",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalAppAppType {
    #[serde(rename = "GOOGLE_CALENDAR")]
    GoogleCalendar,
    #[serde(rename = "GOOGLE_DRIVE")]
    GoogleDrive,
    #[serde(rename = "GMAIL")]
    Gmail,
    #[serde(rename = "SLACK")]
    Slack,
    #[serde(rename = "LINEAR")]
    Linear,
    #[serde(rename = "GITHUB")]
    Github,
    #[serde(rename = "HUBSPOT")]
    Hubspot,
    #[serde(rename = "NOTION")]
    Notion,
    #[serde(rename = "CUSTOM")]
    Custom,
}

impl ExternalAppAppType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoogleCalendar => "GOOGLE_CALENDAR",
            Self::GoogleDrive => "GOOGLE_DRIVE",
            Self::Gmail => "GMAIL",
            Self::Slack => "SLACK",
            Self::Linear => "LINEAR",
            Self::Github => "GITHUB",
            Self::Hubspot => "HUBSPOT",
            Self::Notion => "NOTION",
            Self::Custom => "CUSTOM",
        }
    }
}

impl std::str::FromStr for ExternalAppAppType {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "GOOGLE_CALENDAR" => Ok(Self::GoogleCalendar),
            "GOOGLE_DRIVE" => Ok(Self::GoogleDrive),
            "GMAIL" => Ok(Self::Gmail),
            "SLACK" => Ok(Self::Slack),
            "LINEAR" => Ok(Self::Linear),
            "GITHUB" => Ok(Self::Github),
            "HUBSPOT" => Ok(Self::Hubspot),
            "NOTION" => Ok(Self::Notion),
            "CUSTOM" => Ok(Self::Custom),
            other => Err(crate::UnknownEnumValue {
                enum_name: "ExternalAppAppType",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MCPServerAuthPerformer {
    #[serde(rename = "ADMIN")]
    Admin,
    #[serde(rename = "PER_USER")]
    PerUser,
}

impl MCPServerAuthPerformer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "ADMIN",
            Self::PerUser => "PER_USER",
        }
    }
}

impl std::str::FromStr for MCPServerAuthPerformer {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ADMIN" => Ok(Self::Admin),
            "PER_USER" => Ok(Self::PerUser),
            other => Err(crate::UnknownEnumValue {
                enum_name: "MCPServerAuthPerformer",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MCPServerAuthType {
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "API_TOKEN")]
    ApiToken,
    #[serde(rename = "OAUTH")]
    Oauth,
    #[serde(rename = "PT_OAUTH")]
    PtOauth,
}

impl MCPServerAuthType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::ApiToken => "API_TOKEN",
            Self::Oauth => "OAUTH",
            Self::PtOauth => "PT_OAUTH",
        }
    }
}

impl std::str::FromStr for MCPServerAuthType {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "NONE" => Ok(Self::None),
            "API_TOKEN" => Ok(Self::ApiToken),
            "OAUTH" => Ok(Self::Oauth),
            "PT_OAUTH" => Ok(Self::PtOauth),
            other => Err(crate::UnknownEnumValue {
                enum_name: "MCPServerAuthType",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MCPServerOauthProviderMode {
    #[serde(rename = "AUTO_DISCOVERY")]
    AutoDiscovery,
    #[serde(rename = "KNOWN_PROVIDER")]
    KnownProvider,
}

impl MCPServerOauthProviderMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutoDiscovery => "AUTO_DISCOVERY",
            Self::KnownProvider => "KNOWN_PROVIDER",
        }
    }
}

impl std::str::FromStr for MCPServerOauthProviderMode {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "AUTO_DISCOVERY" => Ok(Self::AutoDiscovery),
            "KNOWN_PROVIDER" => Ok(Self::KnownProvider),
            other => Err(crate::UnknownEnumValue {
                enum_name: "MCPServerOauthProviderMode",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MCPServerStatus {
    #[serde(rename = "CREATED")]
    Created,
    #[serde(rename = "AWAITING_AUTH")]
    AwaitingAuth,
    #[serde(rename = "FETCHING_TOOLS")]
    FetchingTools,
    #[serde(rename = "CONNECTED")]
    Connected,
    #[serde(rename = "DISCONNECTED")]
    Disconnected,
}

impl MCPServerStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::AwaitingAuth => "AWAITING_AUTH",
            Self::FetchingTools => "FETCHING_TOOLS",
            Self::Connected => "CONNECTED",
            Self::Disconnected => "DISCONNECTED",
        }
    }
}

impl std::str::FromStr for MCPServerStatus {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "CREATED" => Ok(Self::Created),
            "AWAITING_AUTH" => Ok(Self::AwaitingAuth),
            "FETCHING_TOOLS" => Ok(Self::FetchingTools),
            "CONNECTED" => Ok(Self::Connected),
            "DISCONNECTED" => Ok(Self::Disconnected),
            other => Err(crate::UnknownEnumValue {
                enum_name: "MCPServerStatus",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MCPServerTransport {
    #[serde(rename = "STDIO")]
    Stdio,
    #[serde(rename = "SSE")]
    Sse,
    #[serde(rename = "STREAMABLE_HTTP")]
    StreamableHttp,
}

impl MCPServerTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "STDIO",
            Self::Sse => "SSE",
            Self::StreamableHttp => "STREAMABLE_HTTP",
        }
    }
}

impl std::str::FromStr for MCPServerTransport {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "STDIO" => Ok(Self::Stdio),
            "SSE" => Ok(Self::Sse),
            "STREAMABLE_HTTP" => Ok(Self::StreamableHttp),
            other => Err(crate::UnknownEnumValue {
                enum_name: "MCPServerTransport",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationNotifType {
    #[serde(rename = "REINDEX")]
    Reindex,
    #[serde(rename = "PERSONA_SHARED")]
    PersonaShared,
    #[serde(rename = "TRIAL_ENDS_TWO_DAYS")]
    TrialEndsTwoDays,
    #[serde(rename = "RELEASE_NOTES")]
    ReleaseNotes,
    #[serde(rename = "ASSISTANT_FILES_READY")]
    AssistantFilesReady,
    #[serde(rename = "FEATURE_ANNOUNCEMENT")]
    FeatureAnnouncement,
    #[serde(rename = "SYSTEM_ANNOUNCEMENT")]
    SystemAnnouncement,
    #[serde(rename = "CONNECTOR_REPEATED_ERRORS")]
    ConnectorRepeatedErrors,
    #[serde(rename = "CONNECTOR_INVALID")]
    ConnectorInvalid,
    #[serde(rename = "LICENSE_EXPIRY_WARNING")]
    LicenseExpiryWarning,
    #[serde(rename = "SCHEDULED_TASK_FAILED")]
    ScheduledTaskFailed,
    #[serde(rename = "SCHEDULED_TASK_AWAITING_APPROVAL")]
    ScheduledTaskAwaitingApproval,
    #[serde(rename = "SCHEDULED_TASK_PRE_APPROVED_ACTION")]
    ScheduledTaskPreApprovedAction,
    #[serde(rename = "APPROVAL_REQUESTED")]
    ApprovalRequested,
}

impl NotificationNotifType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reindex => "REINDEX",
            Self::PersonaShared => "PERSONA_SHARED",
            Self::TrialEndsTwoDays => "TRIAL_ENDS_TWO_DAYS",
            Self::ReleaseNotes => "RELEASE_NOTES",
            Self::AssistantFilesReady => "ASSISTANT_FILES_READY",
            Self::FeatureAnnouncement => "FEATURE_ANNOUNCEMENT",
            Self::SystemAnnouncement => "SYSTEM_ANNOUNCEMENT",
            Self::ConnectorRepeatedErrors => "CONNECTOR_REPEATED_ERRORS",
            Self::ConnectorInvalid => "CONNECTOR_INVALID",
            Self::LicenseExpiryWarning => "LICENSE_EXPIRY_WARNING",
            Self::ScheduledTaskFailed => "SCHEDULED_TASK_FAILED",
            Self::ScheduledTaskAwaitingApproval => "SCHEDULED_TASK_AWAITING_APPROVAL",
            Self::ScheduledTaskPreApprovedAction => "SCHEDULED_TASK_PRE_APPROVED_ACTION",
            Self::ApprovalRequested => "APPROVAL_REQUESTED",
        }
    }
}

impl std::str::FromStr for NotificationNotifType {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "REINDEX" => Ok(Self::Reindex),
            "PERSONA_SHARED" => Ok(Self::PersonaShared),
            "TRIAL_ENDS_TWO_DAYS" => Ok(Self::TrialEndsTwoDays),
            "RELEASE_NOTES" => Ok(Self::ReleaseNotes),
            "ASSISTANT_FILES_READY" => Ok(Self::AssistantFilesReady),
            "FEATURE_ANNOUNCEMENT" => Ok(Self::FeatureAnnouncement),
            "SYSTEM_ANNOUNCEMENT" => Ok(Self::SystemAnnouncement),
            "CONNECTOR_REPEATED_ERRORS" => Ok(Self::ConnectorRepeatedErrors),
            "CONNECTOR_INVALID" => Ok(Self::ConnectorInvalid),
            "LICENSE_EXPIRY_WARNING" => Ok(Self::LicenseExpiryWarning),
            "SCHEDULED_TASK_FAILED" => Ok(Self::ScheduledTaskFailed),
            "SCHEDULED_TASK_AWAITING_APPROVAL" => Ok(Self::ScheduledTaskAwaitingApproval),
            "SCHEDULED_TASK_PRE_APPROVED_ACTION" => Ok(Self::ScheduledTaskPreApprovedAction),
            "APPROVAL_REQUESTED" => Ok(Self::ApprovalRequested),
            other => Err(crate::UnknownEnumValue {
                enum_name: "NotificationNotifType",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSeverity {
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARNING")]
    Warning,
    #[serde(rename = "ERROR")]
    Error,
}

impl NotificationSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

impl std::str::FromStr for NotificationSeverity {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "INFO" => Ok(Self::Info),
            "WARNING" => Ok(Self::Warning),
            "ERROR" => Ok(Self::Error),
            other => Err(crate::UnknownEnumValue {
                enum_name: "NotificationSeverity",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxStatus {
    #[serde(rename = "PROVISIONING")]
    Provisioning,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SLEEPING")]
    Sleeping,
    #[serde(rename = "TERMINATED")]
    Terminated,
    #[serde(rename = "FAILED")]
    Failed,
}

impl SandboxStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "PROVISIONING",
            Self::Running => "RUNNING",
            Self::Sleeping => "SLEEPING",
            Self::Terminated => "TERMINATED",
            Self::Failed => "FAILED",
        }
    }
}

impl std::str::FromStr for SandboxStatus {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "PROVISIONING" => Ok(Self::Provisioning),
            "RUNNING" => Ok(Self::Running),
            "SLEEPING" => Ok(Self::Sleeping),
            "TERMINATED" => Ok(Self::Terminated),
            "FAILED" => Ok(Self::Failed),
            other => Err(crate::UnknownEnumValue {
                enum_name: "SandboxStatus",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserAccountType {
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "BOT")]
    Bot,
    #[serde(rename = "EXT_PERM_USER")]
    ExtPermUser,
    #[serde(rename = "SERVICE_ACCOUNT")]
    ServiceAccount,
    #[serde(rename = "ANONYMOUS")]
    Anonymous,
}

impl UserAccountType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Bot => "BOT",
            Self::ExtPermUser => "EXT_PERM_USER",
            Self::ServiceAccount => "SERVICE_ACCOUNT",
            Self::Anonymous => "ANONYMOUS",
        }
    }
}

impl std::str::FromStr for UserAccountType {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "STANDARD" => Ok(Self::Standard),
            "BOT" => Ok(Self::Bot),
            "EXT_PERM_USER" => Ok(Self::ExtPermUser),
            "SERVICE_ACCOUNT" => Ok(Self::ServiceAccount),
            "ANONYMOUS" => Ok(Self::Anonymous),
            other => Err(crate::UnknownEnumValue {
                enum_name: "UserAccountType",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserDefaultAppMode {
    #[serde(rename = "AUTO")]
    Auto,
    #[serde(rename = "CHAT")]
    Chat,
    #[serde(rename = "SEARCH")]
    Search,
}

impl UserDefaultAppMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Chat => "CHAT",
            Self::Search => "SEARCH",
        }
    }
}

impl std::str::FromStr for UserDefaultAppMode {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "AUTO" => Ok(Self::Auto),
            "CHAT" => Ok(Self::Chat),
            "SEARCH" => Ok(Self::Search),
            other => Err(crate::UnknownEnumValue {
                enum_name: "UserDefaultAppMode",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserReasoningEffortDefault {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
}

impl UserReasoningEffortDefault {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

impl std::str::FromStr for UserReasoningEffortDefault {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "off" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            other => Err(crate::UnknownEnumValue {
                enum_name: "UserReasoningEffortDefault",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    #[serde(rename = "LIMITED")]
    Limited,
    #[serde(rename = "BASIC")]
    Basic,
    #[serde(rename = "ADMIN")]
    Admin,
    #[serde(rename = "CURATOR")]
    Curator,
    #[serde(rename = "GLOBAL_CURATOR")]
    GlobalCurator,
    #[serde(rename = "SLACK_USER")]
    SlackUser,
    #[serde(rename = "EXT_PERM_USER")]
    ExtPermUser,
}

impl UserRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Limited => "LIMITED",
            Self::Basic => "BASIC",
            Self::Admin => "ADMIN",
            Self::Curator => "CURATOR",
            Self::GlobalCurator => "GLOBAL_CURATOR",
            Self::SlackUser => "SLACK_USER",
            Self::ExtPermUser => "EXT_PERM_USER",
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "LIMITED" => Ok(Self::Limited),
            "BASIC" => Ok(Self::Basic),
            "ADMIN" => Ok(Self::Admin),
            "CURATOR" => Ok(Self::Curator),
            "GLOBAL_CURATOR" => Ok(Self::GlobalCurator),
            "SLACK_USER" => Ok(Self::SlackUser),
            "EXT_PERM_USER" => Ok(Self::ExtPermUser),
            other => Err(crate::UnknownEnumValue {
                enum_name: "UserRole",
                value: other.to_string(),
            }),
        }
    }
}

/// Stored as text; the variants are the values the column accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserThemePreference {
    #[serde(rename = "LIGHT")]
    Light,
    #[serde(rename = "DARK")]
    Dark,
    #[serde(rename = "SYSTEM")]
    System,
}

impl UserThemePreference {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "LIGHT",
            Self::Dark => "DARK",
            Self::System => "SYSTEM",
        }
    }
}

impl std::str::FromStr for UserThemePreference {
    type Err = crate::UnknownEnumValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "LIGHT" => Ok(Self::Light),
            "DARK" => Ok(Self::Dark),
            "SYSTEM" => Ok(Self::System),
            other => Err(crate::UnknownEnumValue {
                enum_name: "UserThemePreference",
                value: other.to_string(),
            }),
        }
    }
}

/// Row of `action_approval`.
/// Primary key: approval_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionApproval {
    pub approval_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub actions: serde_json::Value,
    pub app_name: String,
    pub payload: serde_json::Value,
    pub created_at: chrono::NaiveDateTime,
    pub decision: Option<ActionApprovalDecision>,
    pub decided_at: Option<chrono::NaiveDateTime>,
    pub decided_via: Option<ActionApprovalDecidedVia>,
    pub gated_app_id: Option<i32>,
}

impl ActionApproval {
    pub const TABLE: &'static str = "action_approval";
    pub const COLUMNS: &'static [&'static str] = &[
        "approval_id",
        "session_id",
        "actions",
        "app_name",
        "payload",
        "created_at",
        "decision",
        "decided_at",
        "decided_via",
        "gated_app_id",
    ];
}

/// Row of `build_session`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildSession {
    pub id: uuid::Uuid,
    pub user_id: Option<uuid::Uuid>,
    pub name: Option<String>,
    pub status: BuildSessionStatus,
    pub created_at: chrono::NaiveDateTime,
    pub last_activity_at: chrono::NaiveDateTime,
    pub nextjs_port: Option<i32>,
    pub sharing_scope: String,
    pub origin: BuildSessionOrigin,
    pub opencode_session_id: Option<String>,
    pub agent_provider: Option<String>,
    pub agent_model: Option<String>,
    pub skills_hash: Option<String>,
    pub mcp_config_hash: Option<String>,
}

impl BuildSession {
    pub const TABLE: &'static str = "build_session";
    pub const COLUMNS: &'static [&'static str] = &[
        "id",
        "user_id",
        "name",
        "status",
        "created_at",
        "last_activity_at",
        "nextjs_port",
        "sharing_scope",
        "origin",
        "opencode_session_id",
        "agent_provider",
        "agent_model",
        "skills_hash",
        "mcp_config_hash",
    ];
}

/// Row of `external_app`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalApp {
    pub id: i32,
    pub name: String,
    pub app_type: ExternalAppAppType,
    pub enabled: bool,
    pub upstream_url_patterns: Vec<String>,
    pub auth_template: serde_json::Value,
    pub organization_credentials: crate::Encrypted,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl ExternalApp {
    pub const TABLE: &'static str = "external_app";
    pub const COLUMNS: &'static [&'static str] = &[
        "id",
        "name",
        "app_type",
        "enabled",
        "upstream_url_patterns",
        "auth_template",
        "organization_credentials",
        "created_at",
        "updated_at",
    ];
}

/// Row of `gated_app`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatedApp {
    pub id: i32,
    pub external_app_id: Option<i32>,
    pub mcp_server_id: Option<i32>,
}

impl GatedApp {
    pub const TABLE: &'static str = "gated_app";
    pub const COLUMNS: &'static [&'static str] = &["id", "external_app_id", "mcp_server_id"];
}

/// Row of `mcp_server`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MCPServer {
    pub id: i32,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub server_url: String,
    pub transport: Option<MCPServerTransport>,
    pub auth_type: Option<MCPServerAuthType>,
    pub auth_performer: Option<MCPServerAuthPerformer>,
    pub oauth_provider_mode: MCPServerOauthProviderMode,
    pub oauth_authorization_endpoint: Option<String>,
    pub oauth_token_endpoint: Option<String>,
    pub oauth_scopes_override: Option<serde_json::Value>,
    pub oauth_additional_auth_params: Option<serde_json::Value>,
    pub status: MCPServerStatus,
    pub available_in_craft: bool,
    pub admin_connection_config_id: Option<i32>,
    pub is_public: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub last_refreshed_at: Option<chrono::NaiveDateTime>,
}

impl MCPServer {
    pub const TABLE: &'static str = "mcp_server";
    pub const COLUMNS: &'static [&'static str] = &[
        "id",
        "owner",
        "name",
        "description",
        "server_url",
        "transport",
        "auth_type",
        "auth_performer",
        "oauth_provider_mode",
        "oauth_authorization_endpoint",
        "oauth_token_endpoint",
        "oauth_scopes_override",
        "oauth_additional_auth_params",
        "status",
        "available_in_craft",
        "admin_connection_config_id",
        "is_public",
        "created_at",
        "updated_at",
        "last_refreshed_at",
    ];
}

/// Row of `notification`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: i32,
    pub notif_type: NotificationNotifType,
    pub severity: NotificationSeverity,
    pub user_id: Option<uuid::Uuid>,
    pub dismissed: bool,
    pub last_shown: chrono::NaiveDateTime,
    pub first_shown: chrono::NaiveDateTime,
    pub title: String,
    pub description: Option<String>,
    pub additional_data: Option<serde_json::Value>,
}

impl Notification {
    pub const TABLE: &'static str = "notification";
    pub const COLUMNS: &'static [&'static str] = &[
        "id",
        "notif_type",
        "severity",
        "user_id",
        "dismissed",
        "last_shown",
        "first_shown",
        "title",
        "description",
        "additional_data",
    ];
}

/// Row of `sandbox`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sandbox {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub container_id: Option<String>,
    pub status: SandboxStatus,
    pub created_at: chrono::NaiveDateTime,
    pub last_heartbeat: Option<chrono::NaiveDateTime>,
    pub skills_hash: Option<String>,
    pub mcp_config_hash: Option<String>,
    pub encrypted_pat: Option<crate::Encrypted>,
    pub provisioning_attempt_number: i32,
    pub provisioning_started_at: Option<chrono::NaiveDateTime>,
}

impl Sandbox {
    pub const TABLE: &'static str = "sandbox";
    pub const COLUMNS: &'static [&'static str] = &[
        "id",
        "user_id",
        "container_id",
        "status",
        "created_at",
        "last_heartbeat",
        "skills_hash",
        "mcp_config_hash",
        "encrypted_pat",
        "provisioning_attempt_number",
        "provisioning_started_at",
    ];
}

/// Row of `user`.
/// Primary key: id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub role: Option<UserRole>,
    pub account_type: UserAccountType,
    pub craft_enabled: Option<bool>,
    pub prior_emails: Vec<String>,
    pub temperature_override_enabled: Option<bool>,
    pub temperature_default: Option<f64>,
    pub reasoning_effort_default: Option<UserReasoningEffortDefault>,
    pub auto_scroll: Option<bool>,
    pub shortcut_enabled: bool,
    pub theme_preference: Option<UserThemePreference>,
    pub language: String,
    pub chat_background: Option<String>,
    pub default_app_mode: UserDefaultAppMode,
    pub personal_name: Option<String>,
    pub personal_role: Option<String>,
    pub use_memories: bool,
    pub enable_memory_tool: bool,
    pub user_preferences: Option<String>,
    pub chosen_assistants: Option<serde_json::Value>,
    pub visible_assistants: serde_json::Value,
    pub hidden_assistants: serde_json::Value,
    pub effective_permissions: serde_json::Value,
    pub is_group_manager: bool,
    pub oidc_expiry: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub default_model: Option<String>,
    pub paste_as_tile: bool,
    pub voice_auto_send: bool,
    pub voice_auto_playback: bool,
    pub voice_playback_speed: f64,
    pub id: uuid::Uuid,
    pub email: String,
    pub hashed_password: String,
    pub is_active: bool,
    pub is_superuser: bool,
    pub is_verified: bool,
}

impl User {
    pub const TABLE: &'static str = "user";
    pub const COLUMNS: &'static [&'static str] = &[
        "role",
        "account_type",
        "craft_enabled",
        "prior_emails",
        "temperature_override_enabled",
        "temperature_default",
        "reasoning_effort_default",
        "auto_scroll",
        "shortcut_enabled",
        "theme_preference",
        "language",
        "chat_background",
        "default_app_mode",
        "personal_name",
        "personal_role",
        "use_memories",
        "enable_memory_tool",
        "user_preferences",
        "chosen_assistants",
        "visible_assistants",
        "hidden_assistants",
        "effective_permissions",
        "is_group_manager",
        "oidc_expiry",
        "created_at",
        "updated_at",
        "default_model",
        "paste_as_tile",
        "voice_auto_send",
        "voice_auto_playback",
        "voice_playback_speed",
        "id",
        "email",
        "hashed_password",
        "is_active",
        "is_superuser",
        "is_verified",
    ];
}
