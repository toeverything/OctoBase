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

#[derive(Iden)]
pub enum Docs {
    Table,
    Id,
    Workspace,
    Timestamp,
    Blob,
}

#[derive(Iden)]
pub enum OptimizedBlobs {
    Table,
    Workspace,
    Hash,
    Blob,
    Length,
    Timestamp,
    Params,
}

#[derive(Iden)]
pub enum BucketBlobs {
    Table,
    Workspace,
    Hash,
    Length,
    Timestamp,
}
