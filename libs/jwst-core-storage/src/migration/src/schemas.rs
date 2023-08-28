use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Blobs {
    Table,
    #[iden = "workspace_id"]
    Workspace,
    Hash,
    Blob,
    Length,
    #[iden = "created_at"]
    Timestamp,
}

#[derive(Iden)]
pub enum Docs {
    Table,
    Id,
    #[iden = "workspace_id"]
    Workspace,
    Guid,
    IsWorkspace,
    #[iden = "created_at"]
    Timestamp,
    Blob,
}

#[derive(Iden)]
pub enum OptimizedBlobs {
    Table,
    #[iden = "workspace_id"]
    Workspace,
    Hash,
    Blob,
    Length,
    #[iden = "created_at"]
    Timestamp,
    Params,
}

#[derive(Iden)]
pub enum BucketBlobs {
    Table,
    #[iden = "workspace_id"]
    Workspace,
    Hash,
    Length,
    #[iden = "created_at"]
    Timestamp,
}
