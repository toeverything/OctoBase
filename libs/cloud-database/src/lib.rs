// mod model;
mod orm;

pub use orm::Claims;
pub use orm::CloudDatabase;
pub use orm::CreatePermission;
pub use orm::GoogleClaims;
pub use orm::MakeToken;
pub use orm::PermissionType;
pub use orm::RefreshToken;
pub use orm::UpdateWorkspace;
pub use orm::User;
pub use orm::UserCred;
pub use orm::UserLogin;
pub use orm::UserQuery;
pub use orm::UserToken;
pub use orm::UserWithNonce;
pub use orm::WorkspaceDetail;
pub use orm::WorkspaceMetadata;
pub use orm::WorkspaceSearchInput;
pub use orm::WorkspaceWithPermission;

// #[cfg(feature = "postgres")]
// mod postgres;
// #[cfg(feature = "sqlite")]
// mod sqlite;

// pub use model::*;

// #[cfg(feature = "postgres")]
// pub use postgres::PostgreSQL as PostgresDBContext;
// #[cfg(feature = "sqlite")]
// pub use sqlite::SQLite as SqliteDBContext;
