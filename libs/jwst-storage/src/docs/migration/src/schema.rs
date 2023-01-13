use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum UpdateBinary {
    Table,
    Id,
    Workspace,
    Timestamp,
    Blob,
}
