#[cfg(feature = "workspace-search")]
mod indexing;
mod plugins;

use super::*;

#[cfg(feature = "workspace-search")]
pub(super) use indexing::{IndexingPluginImpl, IndexingPluginRegister, IndexingStorageKind};
pub(super) use plugins::{PluginImpl, PluginMap, PluginRegister};

#[cfg(feature = "workspace-search")]
pub use indexing::{SearchBlockItem, SearchBlockList, SearchQueryOptions};
