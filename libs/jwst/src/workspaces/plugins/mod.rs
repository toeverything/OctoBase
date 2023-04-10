#[cfg(feature = "workspace-search")]
mod indexing;
mod plugin;

use super::*;

#[cfg(feature = "workspace-search")]
use indexing::IndexingPluginImpl;
pub(super) use plugin::{PluginImpl, PluginMap, PluginRegister};

#[cfg(feature = "workspace-search")]
pub use indexing::{SearchResult, SearchResults};

/// Setup a [WorkspacePlugin] and insert it into the [Workspace].
/// See [plugins].
fn insert_plugin(
    mut workspace: Workspace,
    config: impl PluginRegister,
) -> Result<Workspace, Box<dyn std::error::Error>> {
    let plugin = config.setup(&mut workspace)?;
    workspace.plugins.insert_plugin(plugin)?;

    Ok(workspace)
}

/// Setup plugin: [indexing]
pub(crate) fn setup_plugin(workspace: Workspace) -> Workspace {
    // default plugins
    if cfg!(feature = "workspace-search") {
        // Set up indexing plugin
        insert_plugin(workspace, indexing::IndexingPluginRegister::default())
            .expect("Failed to setup search plugin")
    } else {
        workspace
    }
}

impl Workspace {
    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    fn update_plugin<P: PluginImpl>(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.update_plugin::<P>(self)
    }

    /// See [plugins].
    fn with_plugin<P: PluginImpl, T>(&self, cb: impl Fn(&P) -> T) -> Option<T> {
        self.plugins.with_plugin::<P, T>(cb)
    }

    #[cfg(feature = "workspace-search")]
    pub fn search<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        // refresh index if doc has update
        self.update_plugin::<IndexingPluginImpl>()?;

        let query = query.as_ref();

        self.with_plugin::<IndexingPluginImpl, Result<SearchResults, Box<dyn std::error::Error>>>(
            |search_plugin| search_plugin.search(query),
        )
        .expect("text search was set up by default")
    }

    #[cfg(feature = "workspace-search")]
    pub fn search_result(&self, query: String) -> String {
        match self.search(query) {
            Ok(list) => serde_json::to_string(&list).unwrap(),
            Err(_) => "[]".to_string(),
        }
    }

    pub fn set_search_index(&self, fields: Vec<String>) -> JwstResult<bool> {
        let fields = fields
            .iter()
            .filter(|f| !f.is_empty())
            .cloned()
            .collect::<Vec<_>>();
        if fields.is_empty() {
            error!("search index cannot be empty");
            return Ok(false);
        }

        let value = serde_json::to_string(&fields).map_err(|_| JwstError::WorkspaceReIndex)?;
        self.retry_with_trx(
            |mut trx| trx.set_metadata(constants::metadata::SEARCH_INDEX, value),
            10,
        )??;
        setup_plugin(self.clone());
        Ok(true)
    }
}
