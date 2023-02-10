#[cfg(feature = "workspace-search")]
mod indexing;
mod plugin;

use super::*;

#[cfg(feature = "workspace-search")]
pub(super) use indexing::IndexingPluginImpl;
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
pub(super) fn setup_plugin(workspace: Workspace) -> Workspace {
    // default plugins
    if cfg!(feature = "workspace-search") {
        // Set up indexing plugin
        insert_plugin(workspace, indexing::IndexingPluginRegister::default())
            .expect("Failed to setup search plugin")
    } else {
        workspace
    }
}
