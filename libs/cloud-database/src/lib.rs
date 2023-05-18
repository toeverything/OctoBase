#[forbid(unsafe_code)]
mod database;
mod entities;
mod model;

pub use database::CloudDatabase;
pub use model::*;

use entities::prelude::*;
use sea_orm::EntityTrait;

type UsersModel = <Users as EntityTrait>::Model;
type UsersActiveModel = entities::users::ActiveModel;
type UsersColumn = <Users as EntityTrait>::Column;
// type WorkspacesModel = <Workspaces as EntityTrait>::Model;
type WorkspacesActiveModel = entities::workspaces::ActiveModel;
type WorkspacesColumn = <Workspaces as EntityTrait>::Column;
type PermissionModel = <Permissions as EntityTrait>::Model;
type PermissionActiveModel = entities::permissions::ActiveModel;
type PermissionColumn = <Permissions as EntityTrait>::Column;
type GoogleUsersModel = <GoogleUsers as EntityTrait>::Model;
type GoogleUsersActiveModel = entities::google_users::ActiveModel;
type GoogleUsersColumn = <GoogleUsers as EntityTrait>::Column;
