//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.3

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "optimized_blobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub hash: String,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))")]
    pub blob: Vec<u8>,
    pub length: i64,
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key, auto_increment = false)]
    pub params: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
