mod history;
pub mod octo;
mod storage;
mod utils;

mod archive {
    mod block;
    mod workspace;

    // TODO: Move search results out of archive
    #[cfg(feature = "workspace-search")]
    pub use workspace::{SearchResult, SearchResults};
    pub use workspace::{Workspace, WorkspaceTransaction};
}

pub use history::{
    parse_history, parse_history_client, BlockHistory, HistoryOperation, RawHistory,
};
pub use log::{error, info};
pub use storage::{BlobMetadata, BlobStorage, DocStorage};
pub use utils::encode_update;

pub use octo::{OctoRead, OctoWorkspace, OctoWorkspaceRef, OctoWrite};

pub(crate) mod prelude {
    //! Consider a prelude mnodule for convenience
    pub use crate::octo::OctoBlockRef;
    pub use crate::octo::{OctoRead, OctoWrite};
    pub use crate::octo::{OctoWorkspace, OctoWorkspaceRef};

    // include prelude for internal maintenance

    pub(crate) use yrs::{self, Doc, Map, MapRef, ReadTxn, TransactionMut};
}
