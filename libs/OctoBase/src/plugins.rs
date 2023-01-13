//! Plugins are an internal experimental interface for extending the [OctoWorkspace].

use super::{OctoReader, OctoWorkspace};
use thiserror::Error;
use type_map::TypeMap;

/// A configuration from which a [OctoPlugin] can be created from.
pub trait OctoPluginRegister {
    type Plugin: OctoPlugin;
    fn setup(
        self,
        workspace: &mut OctoWorkspace,
    ) -> Result<Self::Plugin, Box<dyn std::error::Error>>;
    // Do we need a clean-up thing?
    // -> Box<dyn FnMut(&mut Workspace)>;
}

/// A workspace plugin which comes from a corresponding [OctoPluginRegister::setup].
/// In that setup call, the plugin will have initial access to the whole [OctoWorkspace],
/// and will be able to add listeners to changes to blocks in the [OctoWorkspace].
pub trait OctoPlugin: 'static {
    /// IDEA 1/10:
    /// This update is called sometime between when we know changes have been made to the workspace
    /// and the time when we will get the plugin to query its data (e.g. search()).
    fn on_update(&mut self, _reader: &OctoReader) -> Result<(), Box<dyn std::error::Error>> {
        // Default implementation for a OctoPlugin update does nothing.
        Ok(())
    }
}

/// Internal data structure for holding workspace's plugins.
#[derive(Default)]
pub(crate) struct PluginMap {
    /// We store plugins into the TypeMap, so that their ownership is tied to [OctoWorkspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [OctoWorkspace] experiences, like block updates.
    map: TypeMap,
}

impl PluginMap {
    pub(crate) fn insert_plugin<P: OctoPlugin>(
        &mut self,
        plugin: P,
    ) -> Result<&mut Self, PluginInsertError> {
        if self.get_plugin::<P>().is_some() {
            return Err(PluginInsertError::PluginConflict);
        }

        self.map.insert(plugin);
        Ok(self)
    }

    pub(crate) fn get_plugin<P: OctoPlugin>(&self) -> Option<&P> {
        self.map.get::<P>()
    }

    pub(crate) fn update_plugin<P: OctoPlugin>(
        &mut self,
        reader: &OctoReader,
    ) -> Result<(), OctoPluginUpdateError> {
        let plugin = self
            .map
            .get_mut::<P>()
            .ok_or(OctoPluginUpdateError::PluginNotFound)?;

        plugin.on_update(reader)?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum OctoPluginUpdateError {
    #[error("plugin not found")]
    PluginNotFound,
    #[error("plugin update() returned error")]
    UpdateError(#[from] Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub(crate) enum PluginInsertError {
    PluginConflict,
}
