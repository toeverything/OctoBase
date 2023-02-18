mod metadata;
mod plugins;
mod transaction;
mod workspace;

use super::{info, Block};
use metadata::WorkspaceMetadata;
use plugins::PluginMap;

#[cfg(feature = "workspace-search")]
pub use plugins::{SearchResult, SearchResults};
pub use transaction::WorkspaceTransaction;
pub use workspace::Workspace;
