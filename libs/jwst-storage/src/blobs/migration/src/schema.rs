use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Blobs {
    Table,
    Workspace,
    Hash,
    Blob,
    Length,
    Timestamp,
}
