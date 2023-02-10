mod content;
mod plugins;
mod transaction;
mod workspace;

use super::{info, Block};
use plugins::PluginMap;

pub(crate) use content::Content;

#[cfg(feature = "workspace-search")]
pub use plugins::{SearchResult, SearchResults};
pub use transaction::WorkspaceTransaction;
pub use workspace::Workspace;
