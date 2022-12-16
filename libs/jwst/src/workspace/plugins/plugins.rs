//! Plugins are an internal experimental interface for extending the [Workspace].

use super::*;
use type_map::TypeMap;

/// A configuration from which a [WorkspacePlugin] can be created from.
pub(crate) trait PluginRegister {
    type Plugin: PluginImpl;
    // Do we need self?
    fn setup(self, ws: &mut Workspace) -> Result<Self::Plugin, Box<dyn std::error::Error>>;
    // Do we need a clean-up thing?
    // -> Box<dyn FnMut(&mut Workspace)>;
}

/// A workspace plugin which comes from a corresponding [WorkspacePluginConfig::setup].
/// In that setup call, the plugin will have initial access to the whole [Workspace],
/// and will be able to add listeners to changes to blocks in the [Workspace].
pub(crate) trait PluginImpl: 'static {
    /// IDEA 1/10:
    /// This update is called sometime between when we know changes have been made to the workspace
    /// and the time when we will get the plugin to query its data (e.g. search())
    fn on_update(&mut self, _ws: &Content) -> Result<(), Box<dyn std::error::Error>> {
        // Default implementation for a WorkspacePlugin update does nothing.
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct PluginMap {
    /// We store plugins into the TypeMap, so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    map: TypeMap,
}

impl PluginMap {
    pub(crate) fn insert_plugin<P: PluginImpl>(
        &mut self,
        plugin: P,
    ) -> Result<&mut Self, Box<dyn std::error::Error>> {
        self.map.insert(plugin);
        Ok(self)
    }

    pub(crate) fn get_plugin<P: PluginImpl>(&self) -> Option<&P> {
        self.map.get::<P>()
    }

    pub(crate) fn get_plugin_mut<P: PluginImpl>(&mut self) -> Option<&mut P> {
        self.map.get_mut::<P>()
    }

    pub(crate) fn update_plugin<P: PluginImpl>(
        &mut self,
        content: &Content,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let plugin = self.map.get_mut::<P>().ok_or("Plugin not found")?;

        plugin.on_update(content)?;

        Ok(())
    }
}
