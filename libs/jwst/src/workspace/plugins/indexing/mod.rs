mod indexing;
mod register;
mod types;

use super::{Content, PluginImpl, PluginRegister, Workspace};

pub use indexing::IndexingPluginImpl;
pub(super) use register::IndexingPluginRegister;
pub use types::{SearchBlockItem, SearchBlockList, SearchQueryOptions};
