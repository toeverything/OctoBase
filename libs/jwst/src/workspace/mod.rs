mod content;
mod plugins;
mod workspace;

use super::Block;
use plugins::PluginMap;

pub(crate) use content::Content;

#[cfg(feature = "workspace-search")]
pub use plugins::{SearchBlockItem, SearchBlockList, SearchQueryOptions};
pub use workspace::{Workspace, WorkspaceTransaction};
