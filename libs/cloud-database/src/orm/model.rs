// use super::*;
// use sqlx::{postgres::PgRow, FromRow, Result, Row};

use affine_cloud_migration::DbErr;
use chrono::naive::serde::{ts_milliseconds, ts_seconds};
use jwst_logger::error;
use schemars::{JsonSchema, JsonSchema_repr};
use sea_orm::{FromQueryResult, TryGetable};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::{self, types::chrono::NaiveDateTime, FromRow, Type};
use yrs::Map;

// impl FromRow<'_, PgRow> for Member {
//     fn from_row(row: &PgRow) -> Result<Self> {
//         let id = row.try_get("id")?;
//         let accepted = row.try_get("accepted")?;
//         let type_ = row.try_get("type")?;
//         let created_at = row.try_get("created_at")?;

//         let user = if let Some(email) = row.try_get("user_email")? {
//             UserCred::UnRegistered { email }
//         } else {
//             let id = row.try_get("user_id")?;
//             let name = row.try_get("user_name")?;
//             let email = row.try_get("user_table_email")?;
//             let avatar_url = row.try_get("avatar_url")?;
//             let created_at = row.try_get("user_created_at")?;
//             UserCred::Registered(User {
//                 id,
//                 name,
//                 email,
//                 avatar_url,
//                 created_at,
//             })
//         };

//         Ok(Member {
//             id,
//             accepted,
//             user,
//             type_,
//             created_at,
//         })
//     }
// }

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
#[repr(i32)]
pub enum WorkspaceType {
    Private = 0,
    Normal = 1,
}

impl From<i32> for WorkspaceType {
    fn from(i: i32) -> Self {
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
    fn try_get(
        res: &sea_orm::QueryResult,
        pre: &str,
        col: &str,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i32 = res
            .try_get(pre, col)
            .map_err(|e| sea_orm::TryGetError::DbErr(e))?;
        Ok(WorkspaceType::from(i))
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
    pub id: String,
    pub public: bool,
    #[serde(rename = "type")]
    pub r#type: WorkspaceType,
    #[serde(with = "ts_milliseconds")]
    #[schemars(with = "i64")]
    pub created_at: NaiveDateTime,
    pub permission: PermissionType,
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
#[repr(i32)]
pub enum PermissionType {
    Read = 0,
    Write = 1,
    Admin = 10,
    Owner = 99,
}

impl From<i32> for PermissionType {
    fn from(i: i32) -> Self {
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
    fn try_get(
        res: &sea_orm::QueryResult,
        pre: &str,
        col: &str,
    ) -> Result<Self, sea_orm::TryGetError> {
        let i: i32 = res
            .try_get(pre, col)
            .map_err(|e| sea_orm::TryGetError::DbErr(e))?;
        Ok(PermissionType::from(i))
    }
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
    // .column_as(UsersColumn::AvatarUrl, "avatar_url")
    // .column_as(UsersColumn::CreatedAt, "user_created_at")
    pub id: i64,
    pub r#type: PermissionType,
    pub accepted: bool,
    pub created_at: NaiveDateTime,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar_url: Option<String>,
    pub user_created_at: Option<NaiveDateTime>,
}

impl From<&MemberResult> for Member {
    fn from(r: &MemberResult) -> Self {
        let user = if let Some(id) = r.user_id.clone() {
            UserCred::Registered(User {
                id: id.clone(),
                name: r.user_name.clone().unwrap(),
                email: r.user_email.clone().unwrap(),
                avatar_url: r.user_avatar_url.clone(),
                created_at: r.user_created_at.clone().unwrap(),
            })
        } else {
            UserCred::UnRegistered {
                email: r.user_email.clone().unwrap(),
            }
        };
        Member {
            id: r.id,
            user,
            accepted: r.accepted,
            r#type: r.r#type.clone(),
            created_at: r.created_at,
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
