use chrono::naive::serde::{ts_milliseconds, ts_seconds};
use schemars::{JsonSchema, JsonSchema_repr};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::{self, types::chrono::NaiveDateTime, FromRow, Type};
use yrs::Map;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GoogleClaims {
    // name of project
    pub aud: String,
    pub auth_time: usize,
    pub email: String,
    pub email_verified: bool,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub name: String,
    // picture of avatar
    pub picture: String,
    pub sub: String,
    pub user_id: String,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserWithNonce {
    #[sqlx(flatten)]
    pub user: User,
    pub token_nonce: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserQuery {
    pub email: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Claims {
    #[serde(with = "ts_seconds")]
    #[schemars(with = "i64")]
    pub exp: NaiveDateTime,
    #[serde(flatten)]
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum MakeToken {
    User(UserLogin),
    Refresh { token: String },
    Google { token: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserLogin {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateUser {
    pub name: String,
    pub avatar_url: Option<String>,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserToken {
    pub token: String,
    pub refresh: String,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefreshToken {
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub expires: NaiveDateTime,
    pub user_id: i32,
    pub token_nonce: i16,
}

#[derive(
    Type, Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug, Clone, Copy, JsonSchema_repr,
)]
#[repr(i16)]
pub enum WorkspaceType {
    Private = 0,
    Normal = 1,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Workspace {
    pub id: String,
    pub public: bool,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub type_: WorkspaceType,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceWithPermission {
    pub permission: PermissionType,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceDetail {
    // None if it's private
    pub owner: Option<User>,
    pub member_count: i64,
    #[serde(flatten)]
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateWorkspace {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceSearchInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceSearchResults {
    pub items: Vec<WorkspaceSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceSearchResult {
    pub block_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateWorkspace {
    pub public: bool,
}

#[derive(
    Type,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Clone,
    JsonSchema_repr,
)]
#[repr(i16)]
pub enum PermissionType {
    Read = 0,
    Write = 1,
    Admin = 10,
    Owner = 99,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Permission {
    pub id: i64,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub type_: PermissionType,
    pub workspace_id: String,
    pub user_id: Option<i32>,
    pub user_email: Option<String>,
    pub accepted: bool,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

impl PermissionType {
    pub fn can_write(&self) -> bool {
        *self >= Self::Write
    }

    pub fn can_admin(&self) -> bool {
        *self >= Self::Admin
    }

    pub fn is_owner(&self) -> bool {
        *self == Self::Owner
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreatePermission {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum UserCred {
    Registered(User),
    UnRegistered { email: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Member {
    pub id: i64,
    pub user: UserCred,
    pub accepted: bool,
    #[serde(rename = "type")]
    pub type_: PermissionType,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserInWorkspace {
    #[serde(flatten)]
    pub user: UserCred,
    pub in_workspace: bool,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Exist {
    pub exists: bool,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Id {
    pub id: i32,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BigId {
    pub id: i64,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Count {
    pub count: i64,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceMetadata {
    pub name: String,
}

impl WorkspaceMetadata {
    pub fn parse(metadata: &Map) -> Option<Self> {
        let name = metadata.get("name")?.to_string();

        Some(WorkspaceMetadata { name })
    }
}
