// use super::*;
// use sqlx::{postgres::PgRow, FromRow, Result, Row};

use chrono::naive::serde::{ts_milliseconds, ts_seconds};
use chrono::{DateTime, Utc};
use jwst_logger::error;
use schemars::{JsonSchema, JsonSchema_repr};
use sea_orm::{FromQueryResult, TryGetable};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::{self, types::chrono::NaiveDateTime, FromRow, Type};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserInfo {
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    // picture of avatar
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FirebaseClaims {
    // name of project
    pub aud: String,
    pub auth_time: usize,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub sub: String,
    pub user_id: String,
    #[serde(flatten)]
    pub user_info: Option<UserInfo>,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserWithNonce {
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
    DebugCreateUser(CreateUser),
    DebugLoginUser(UserLogin),
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
    pub user_id: String,
    pub token_nonce: i16,
}

#[derive(
    Type, Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug, Clone, Copy, JsonSchema_repr,
)]
#[repr(i32)]
pub enum WorkspaceType {
    Private = 0,
    Normal = 1,
}

impl From<i16> for WorkspaceType {
    fn from(i: i16) -> Self {
        match i {
            0 => WorkspaceType::Private,
            1 => WorkspaceType::Normal,
            _ => {
                error!("invalid workspace type: {}", i);
                WorkspaceType::Private
            }
        }
    }
}

impl TryGetable for WorkspaceType {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i16 = res.try_get_by(index).map_err(sea_orm::TryGetError::DbErr)?;
        Ok(WorkspaceType::from(i))
    }

    fn try_get(
        res: &sea_orm::QueryResult,
        pre: &str,
        col: &str,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i16 = res.try_get(pre, col).map_err(sea_orm::TryGetError::DbErr)?;
        Ok(WorkspaceType::from(i))
    }

    fn try_get_by_index(
        res: &sea_orm::QueryResult,
        index: usize,
    ) -> Result<Self, sea_orm::TryGetError> {
        Self::try_get_by(res, index)
    }
}

#[derive(FromQueryResult, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Workspace {
    pub id: String,
    pub public: bool,
    #[serde(rename = "type")]
    pub r#type: WorkspaceType,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromQueryResult, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceWithPermission {
    pub permission: PermissionType,
    // #[serde(flatten)]
    // pub workspace: Workspace,
    pub id: String,
    pub public: bool,
    #[serde(rename = "type")]
    pub r#type: WorkspaceType,
    // #[serde(with = "ts_milliseconds")]
    // #[schemars(with = "i64")]
    // pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceDetail {
    // None if it's private
    pub owner: Option<User>,
    pub member_count: u64,
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

impl From<i16> for PermissionType {
    fn from(i: i16) -> Self {
        match i {
            0 => PermissionType::Read,
            1 => PermissionType::Write,
            10 => PermissionType::Admin,
            99 => PermissionType::Owner,
            _ => {
                error!("invalid permission type: {}", i);
                PermissionType::Read
            }
        }
    }
}

impl TryGetable for PermissionType {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i16 = res.try_get_by(index).map_err(sea_orm::TryGetError::DbErr)?;
        Ok(PermissionType::from(i))
    }

    fn try_get(
        res: &sea_orm::QueryResult,
        pre: &str,
        col: &str,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i16 = res.try_get(pre, col).map_err(sea_orm::TryGetError::DbErr)?;
        Ok(PermissionType::from(i))
    }

    fn try_get_by_index(
        res: &sea_orm::QueryResult,
        index: usize,
    ) -> Result<Self, sea_orm::TryGetError> {
        Self::try_get_by(res, index)
    }
}

#[derive(FromRow, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Permission {
    pub id: String,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub r#type: PermissionType,
    pub workspace_id: String,
    pub user_id: Option<String>,
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
    pub id: String,
    pub user: UserCred,
    pub accepted: bool,
    #[serde(rename = "type")]
    pub r#type: PermissionType,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
}

#[derive(FromQueryResult)]
pub struct MemberResult {
    // .column_as(PermissionColumn::Id, "id")
    // .column_as(PermissionColumn::Type, "type")
    // .column_as(PermissionColumn::UserEmail, "user_email")
    // .column_as(PermissionColumn::Accepted, "accepted")
    // .column_as(PermissionColumn::CreatedAt, "created_at")
    // .column_as(UsersColumn::Id, "user_id")
    // .column_as(UsersColumn::Name, "user_name")
    // .column_as(UsersColumn::Email, "user_table_email")
    // .column_as(UsersColumn::AvatarUrl, "user_avatar_url")
    // .column_as(UsersColumn::CreatedAt, "user_created_at")
    pub id: String,
    pub r#type: PermissionType,
    pub user_email: Option<String>,
    pub accepted: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub user_table_email: Option<String>,
    pub user_avatar_url: Option<String>,
    pub user_created_at: Option<DateTime<Utc>>,
}

impl From<&MemberResult> for Member {
    fn from(r: &MemberResult) -> Self {
        let user = if let Some(id) = r.user_id.clone() {
            UserCred::Registered(User {
                id,
                name: r.user_name.clone().unwrap(),
                email: r.user_table_email.clone().unwrap(),
                avatar_url: r.user_avatar_url.clone(),
                created_at: r.user_created_at.unwrap_or_default().naive_local(),
            })
        } else {
            UserCred::UnRegistered {
                email: r.user_email.clone().unwrap(),
            }
        };
        Member {
            id: r.id.clone(),
            user,
            accepted: r.accepted,
            r#type: r.r#type.clone(),
            created_at: r.created_at.unwrap_or_default().naive_local(),
        }
    }
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
