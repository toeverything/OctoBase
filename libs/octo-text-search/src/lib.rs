//! # OctoBase Text Search Plugin
//!
//! Use [`TextSearchPluginConfig`]`::default()`
mod indexing;
mod register;

pub use indexing::{TextSearchPlugin, TextSearchResult, TextSearchResults};
use jwst::OctoWorkspace;
pub use register::TextSearchPluginConfig;

pub trait OctoTextSearch {
    /// Added for [OctoWorkspace] with octo-text-search crate.
    fn search<S: AsRef<str>>(
        &mut self,
        options: S,
    ) -> Result<TextSearchResults, Box<dyn std::error::Error>>;
}

impl OctoTextSearch for OctoWorkspace {
    fn search<S: AsRef<str>>(
        &mut self,
        options: S,
    ) -> Result<TextSearchResults, Box<dyn std::error::Error>> {
        // refresh index if doc has update
        self.update_plugin::<TextSearchPlugin>()?;

        let search_plugin = self
            .get_plugin::<TextSearchPlugin>()
            .expect("text search was set up by default");
        search_plugin.search(options.as_ref())
    }
}
