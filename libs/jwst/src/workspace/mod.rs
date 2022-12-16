mod content;
mod plugins;
mod workspace;

use super::{info, Block};

#[cfg(feature = "workspace-search")]
use plugins::{IndexingPluginImpl, IndexingPluginRegister, IndexingStorageKind};
use plugins::{PluginImpl, PluginMap, PluginRegister};

pub(crate) use content::Content;

#[cfg(feature = "workspace-search")]
pub use plugins::{SearchBlockItem, SearchBlockList, SearchQueryOptions};
pub use workspace::{Workspace, WorkspaceTransaction};
