mod content;
#[cfg(feature = "workspace-search")]
mod indexing;
mod plugins;
mod workspace;

use super::{info, Block};
#[cfg(feature = "workspace-search")]
use indexing::{
    WorkspaceTextSearchPlugin, WorkspaceTextSearchPluginConfig,
    WorkspaceTextSearchPluginConfigStorageKind,
};
use plugins::{WorkspacePlugin, WorkspacePluginConfig, WorkspacePluginMap};

pub(crate) use content::WorkspaceContent;
#[cfg(feature = "workspace-search")]
pub use indexing::{SearchBlockItem, SearchBlockList, SearchQueryOptions};
pub use workspace::{Workspace, WorkspaceTransaction};
