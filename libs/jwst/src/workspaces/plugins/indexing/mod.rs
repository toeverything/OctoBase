mod indexer;
mod register;
mod tokenizer;

use super::{PluginImpl, PluginRegister, Workspace};
use tokenizer::{tokenizers_register, LANG_CN};

pub use indexer::{IndexingPluginImpl, SearchResult, SearchResults};
pub(super) use register::IndexingPluginRegister;
