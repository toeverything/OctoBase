use chrono::naive::serde::{ts_milliseconds, ts_seconds};
use serde::{Deserialize, Serialize};
use schemars::{JsonSchema};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::{self, types::chrono::NaiveDateTime, FromRow, Type};
use yrs::Map;

#[derive(Debug, Deserialize)]
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

#[derive(FromRow, Serialize, Debug, Deserialize, JsonSchema)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromRow)]
pub struct UserWithNonce {
    #[sqlx(flatten)]
    pub user: User,
    pub token_nonce: i16,
}

#[derive(Deserialize)]
pub struct UserQuery {
    pub email: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    #[serde(with = "ts_seconds")]
    pub exp: NaiveDateTime,
    #[serde(flatten)]
    pub user: User,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum MakeToken {
    User(UserLogin),
    Refresh { token: String },
    Google { token: String },
}

#[derive(Deserialize)]
pub struct UserLogin {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub name: String,
    pub avatar_url: Option<String>,
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserToken {
    pub token: String,
    pub refresh: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct RefreshToken {
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub expires: NaiveDateTime,
    pub user_id: i32,
    pub token_nonce: i16,
}

#[derive(Type, Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug, Clone, Copy, JsonSchema)]
#[repr(i16)]
pub enum WorkspaceType {
    Private = 0,
    Normal = 1,
}

#[derive(FromRow, Serialize, Debug, Clone, JsonSchema)]
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

#[derive(FromRow, Serialize, Debug, Clone, JsonSchema)]
pub struct WorkspaceWithPermission {
    pub permission: PermissionType,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub workspace: Workspace,
}

#[derive(Serialize, Debug, JsonSchema)]
pub struct WorkspaceDetail {
    // None if it's private
    pub owner: Option<User>,
    pub member_count: i64,
    #[serde(flatten)]
    pub workspace: Workspace,
}

#[derive(Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
    pub avatar: String,
}

#[derive(Deserialize)]
pub struct WorkspaceSearchInput {
    pub query: String,
}

#[derive(Serialize)]
pub struct WorkspaceSearchResults {
    pub items: Vec<WorkspaceSearchResult>,
}

#[derive(Serialize)]
pub struct WorkspaceSearchResult {
    pub block_id: String,
}

#[derive(Deserialize)]
pub struct UpdateWorkspace {
    pub public: bool,
}

#[derive(Type, Serialize_repr, Deserialize_repr, PartialEq, Eq, PartialOrd, Ord, Debug, Clone, JsonSchema)]
#[repr(i16)]
pub enum PermissionType {
    Read = 0,
    Write = 1,
    Admin = 10,
    Owner = 99,
}

#[derive(FromRow, Serialize)]
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

#[derive(Deserialize)]
pub struct CreatePermission {
    pub email: String,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum UserCred {
    Registered(User),
    UnRegistered { email: String },
}

#[derive(Serialize)]
pub struct Member {
    pub id: i64,
    pub user: UserCred,
    pub accepted: bool,
    pub type_: PermissionType,
    #[serde(with = "ts_milliseconds")]
    pub created_at: NaiveDateTime,
}

#[derive(Serialize)]
pub struct UserInWorkspace {
    #[serde(flatten)]
    pub user: UserCred,
    pub in_workspace: bool,
}

#[derive(FromRow)]
pub struct Exist {
    pub exists: bool,
}

#[derive(FromRow)]
pub struct Id {
    pub id: i32,
}

#[derive(FromRow)]
pub struct BigId {
    pub id: i64,
}

#[derive(FromRow)]
pub struct Count {
    pub count: i64,
}

pub struct WorkspaceMetadata {
    pub name: String,
    pub avatar: String,
}

impl WorkspaceMetadata {
    pub fn parse(metadata: &Map) -> Option<Self> {
        let name = metadata.get("name")?.to_string();
        let avatar = metadata.get("avatar")?.to_string();

        Some(WorkspaceMetadata { name, avatar })
    }
}
