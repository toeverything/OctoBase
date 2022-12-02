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
    pub created_at: NaiveDateTime,
}

#[derive(FromRow, Serialize)]
pub struct WorkspaceWithPermission {
    pub permission: PermissionType,
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
    pub workspace_id: i32,
    // TODO: currently default to write
    // #[serde(rename = "type")]
    // pub type_: PermissionType,
}

#[derive(FromRow)]
pub struct Exist {
    pub exists: bool,
}

#[derive(FromRow, Serialize)]
pub struct ShareUrl {
    pub id: i32,
    pub workspace_id: i32,
    pub url: String,
    pub created_at: NaiveDateTime,
}
