mod indexing;
mod register;
mod tokenizer;

use super::{Content, PluginImpl, PluginRegister, Workspace};
use tokenizer::{tokenizers_register, LANG_CN};

pub use indexing::{IndexingPluginImpl, SearchResults};
pub(super) use register::IndexingPluginRegister;
