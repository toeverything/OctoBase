mod database;
mod entities;
mod model;

pub use database::ORM;

use entities::prelude::*;
use sea_orm::EntityTrait;

type UsersModel = <Users as EntityTrait>::Model;
type UsersActiveModel = entities::users::ActiveModel;
type UsersColumn = <Users as EntityTrait>::Column;
type WorkspacesModel = <Workspaces as EntityTrait>::Model;
type WorkspacesActiveModel = entities::workspaces::ActiveModel;
type WorkspacesColumn = <Workspaces as EntityTrait>::Column;
type PermissionModel = <Permission as EntityTrait>::Model;
type PermissionActiveModel = entities::permission::ActiveModel;
type PermissionColumn = <Permission as EntityTrait>::Column;
