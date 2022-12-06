use chrono::naive::serde::{ts_milliseconds, ts_seconds};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::{self, types::chrono::NaiveDateTime, FromRow, Type};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
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

#[derive(Serialize)]
pub struct User {}

#[derive(Type, Serialize_repr, Deserialize_repr)]
#[repr(i16)]
pub enum WorkspaceType {
    Private = 0,
    Normal = 1,
}

#[derive(FromRow, Serialize)]
pub struct Workspace {
    pub id: i32,
    pub public: bool,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub type_: WorkspaceType,
    #[serde(with = "ts_milliseconds")]
    pub created_at: NaiveDateTime,
}

#[derive(FromRow, Serialize)]
pub struct WorkspaceWithPermission {
    pub permission: PermissionType,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub workspace: Workspace,
}

#[derive(FromRow, Serialize)]
pub struct WorkspaceDetail {
    pub owner: String,
    pub member_count: i32,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub workspace: Workspace,
}

#[derive(Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateWorkspace {
    pub public: bool,
}

#[derive(Type, Serialize_repr, Deserialize_repr, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i16)]
pub enum PermissionType {
    Read = 0,
    Write = 1,
    Admin = 2,
    Owner = 3,
}

#[derive(Type, Serialize_repr, Deserialize_repr, Clone, Copy)]
#[repr(i16)]
pub enum UserCredType {
    Id = 0,
    Email = 1,
}

#[derive(FromRow, Serialize)]
pub struct Permission {
    pub id: i32,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub type_: PermissionType,
    pub workspace_id: i32,
    pub user_cred: String,
    pub user_cred_type: UserCredType,
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
}

#[derive(Deserialize)]
pub struct CreatePermission {
    pub user_cred: String,
    pub user_cred_type: UserCredType,
    // TODO: currently default to write
    // #[serde(rename = "type")]
    // pub type_: PermissionType,
}

#[derive(Serialize)]
pub struct Member {
    pub user: Option<User>,
    pub accepted: bool,
    #[serde(rename = "type")]
    pub type_: PermissionType,
}

#[derive(Deserialize, Serialize)]
pub struct Invitation {
    pub id: i32,
    #[serde(with = "ts_seconds")]
    pub expire: NaiveDateTime,
}

#[derive(FromRow)]
pub struct Exist {
    pub exists: bool,
}

#[derive(FromRow)]
pub struct Id {
    pub id: i32,
}
