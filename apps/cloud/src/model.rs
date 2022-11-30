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
    Personal = 0,
    Team = 1,
}

#[derive(FromRow, Serialize)]
pub struct Workspace {
    id: i32,
    owner: String,
    public: bool,
    name: String,
    avatar_url: Option<String>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    type_: WorkspaceType,
    created_at: NaiveDateTime,
}

#[derive(Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: WorkspaceType,
    pub public: bool,
}

#[derive(Type, Serialize_repr, Deserialize_repr)]
#[repr(i16)]
pub enum PermissionType {
    Read = 0,
    Write = 1,
}

#[derive(Deserialize)]
pub struct CreatePermission {
    pub workspace_id: i32,
    #[serde(rename = "type")]
    pub type_: PermissionType,
}
