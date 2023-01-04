mod content;
mod plugins;
mod workspace;

use plugins::PluginMap;

pub(crate) use content::Content;

#[cfg(feature = "workspace-search")]
pub use plugins::{SearchResult, SearchResults};
pub use workspace::{Workspace, WorkspaceTransaction};
