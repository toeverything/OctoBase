mod block;
mod history;
pub mod octo;
mod storage;
mod utils;
mod workspace;

pub use history::{
    parse_history, parse_history_client, BlockHistory, HistoryOperation, RawHistory,
};
pub use log::{error, info};
pub use storage::{BlobMetadata, BlobStorage, DocStorage};
pub use utils::encode_update;

pub use octo::{OctoRead, OctoWorkspace, OctoWorkspaceRef, OctoWrite};

#[cfg(feature = "workspace-search")]
pub use workspace::{SearchResult, SearchResults};
pub use workspace::{Workspace, WorkspaceTransaction};
