mod indexing;
mod register;
mod tokenizer;

use super::{PluginImpl, PluginRegister, Workspace};
use tokenizer::{tokenizers_register, LANG_CN};

pub use indexing::{IndexingPluginImpl, SearchResult, SearchResults};
pub(super) use register::IndexingPluginRegister;
