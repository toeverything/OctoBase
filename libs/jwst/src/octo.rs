//! Next iteration of the OctoBase design.
//! Progress 0/10:
//!
//! This is a clean rewrite of OctoBase concepts following the upgrade
//! of yrs 0.14 to support subdocs.
//!
//! At this moment, everything is in one file to create a pressure not to
//! over-complicate or over-expand the surface area of OctoBase core.
use lib0::any::Any;
use std::{borrow::Cow, collections::HashMap, rc::Rc};
use thiserror::Error;
use yrs::{ArrayPrelim, Map, MapPrelim, MapRef, ReadTxn, Transaction, TransactionMut};

pub mod concepts {
    //! Linkable shared documentation of different OctoBase concepts.
    //!
    //! Progress 1/10:
    //!
    //!  * I like that we can have a single place to define shared concepts
    //!  * This might be bad if people searching the `cargo doc` find these
    //!    pages and fail to find the actual source code of JWST.
    //!  * Consider moving concepts to their own `octo_concept`s crate to avoid that issue.

    /// When you have a reference to a YJS block, you are usually going to be
    /// interacting with a set of [SysProp]s or [UserDefinedProp]s.
    /// ```json
    /// { "sys:flavor": "affine:text",
    ///   "sys:created": 946684800000,
    ///   "sys:children": ["block1", "block2"],
    ///   "prop:text": "123",
    ///   "prop:color": "#ff0000" }
    /// ```
    ///
    /// Also see [YJSBlockHistory].
    pub struct YJSBlockPropMap;

    /// The built-in properties OctoBase manages on the [YJSBlockPropMap]. For example: `"sys:flavor"` ([SysPropFlavor]), `"sys:created"`, `"sys:children"`.
    ///
    /// ```json
    /// "sys:flavor": "affine:text",
    /// "sys:created": 946684800000,
    /// "sys:children": ["block1", "block2"],
    /// ```
    pub struct SysProp;

    /// "Flavors" are specified as a [SysProp] named `"sys:flavor"` of our [YJSBlockPropMap].
    ///
    /// Flavor is the type of `Block`, which is derived from [particle physics],
    /// which means that all blocks have the same basic properties, when the flavor of
    /// `Block` changing, the basic attributes will not change, but the interpretation
    /// of user-defined attributes will be different.
    ///
    /// [particle physics]: https://en.wikipedia.org/wiki/Flavour_(particle_physics)
    pub struct SysPropFlavor;

    /// User defined attributes on the [YJSBlockPropMap]. For example: `"prop:xywh"`, `"prop:text"`, etc.
    ///
    /// ### Common props
    ///
    /// These props are understood by the workspace search plugin, for example.
    ///
    /// ```json
    /// "prop:text": "123",
    /// "prop:title": "123"
    /// ```
    ///
    /// ## AFFiNE examples
    ///
    /// Some examples for different AFFiNE [SysPropFlavor]s are as follows:
    ///
    /// ### `"sys:flavor": "affine:shape"`
    /// ```json
    /// "prop:xywh": "[0,0,720,480]",
    /// "prop:type": "rectangle",
    /// "prop:color": "black",
    /// ```
    ///
    /// ### `"sys:flavor": "affine:list"`
    /// ```json
    ///  "prop:checked": false,
    ///  "prop:text": "List item text",
    ///  "prop:type": "bulleted",
    /// ```
    pub struct UserDefinedProp;

    /// Each time an edit or update happens to a Block, an event is inserted into the
    /// YJS Array this points to.
    ///
    /// The values in this array each look like an array of three items: `[operator, utc_timestamp_ms, "action"]`
    /// ```json
    /// [ [35, 1672374542865, "add"],
    ///   [35, 1672374543214, "update"],
    ///   [35, 1672378928393, "update"] ]
    /// ```
    ///
    /// Also see [YJSBlockPropMap] and [crate::HistoryOperation].
    pub struct YJSBlockHistory;
}

mod plugins {
    //! Plugins are an internal experimental interface for extending the [OctoWorkspace].

    use super::{OctoReader, OctoWorkspace};
    use thiserror::Error;
    use type_map::TypeMap;

    /// A configuration from which a [OctoPlugin] can be created from.
    pub(crate) trait OctoPluginRegister {
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
    pub(crate) trait OctoPlugin: 'static {
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
        ) -> Result<&mut Self, OctoPluginInsertError> {
            if self.get_plugin::<P>().is_some() {
                return Err(OctoPluginInsertError::PluginConflict);
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

    #[derive(Error, Debug)]
    pub enum OctoPluginInsertError {
        #[error("plugin of type already exists")]
        PluginConflict,
    }
}

/// See [OctoWorkspace] and [OctoWorkspaceContent].
#[derive(Clone, Debug, PartialEq)]
pub struct OctoWorkspaceRef {
    id: Rc<str>,
    // Perhaps we should include a way to
    // identify the operator / client id
    // via the Workspace's Awareness
    /// Indexed by "block id", the values are the corresponding block's [concepts::YJSBlockPropMap]s.
    yrs_block_ref: yrs::MapRef,
    /// Indexed by "block id", the values are the corresponding block's [concepts::YJSBlockHistory].
    yrs_updated_ref: yrs::MapRef,
    /// Used for assorted attributes of the workspace like the Workspace's icon or display name.
    yrs_metadata_ref: MapRef,
}

/// A reference to a Block in a Workspace.
#[derive(Debug, PartialEq)]
pub struct OctoBlockRef {
    /// Unique Block ID
    id: Rc<str>,
    /// Retain workspace reference to improve troubleshooting if someone tries to use an [OctoBlockRef]
    /// from one [OctoWorkspaceRef] in a different [OctoWorkspaceRef].
    workspace_ref: OctoWorkspaceRef,
    /// This YJS Map holds the data for keys such as
    /// ```json
    /// { "sys:flavor": "affine:text",
    ///   "sys:created": 946684800000,
    ///   "sys:children": ["block1", "block2"],
    ///   "prop:text": "123",
    ///   "prop:color": "#ff0000" }
    /// ```
    /// See [concepts::YJSBlockPropMap].
    ///
    /// Internal reference directly to the [yrs::MapRef] in the "Workspace"
    yrs_props_ref: yrs::MapRef,
    /// Internal reference directly to the [yrs::ArrayRef] in the "Workspace" which holds
    /// a list of changes that have been applied to the Block over time.
    /// See [concepts::YJSBlockHistory].
    yrs_history_ref: yrs::ArrayRef,
}

/// A reference to a Block in a Workspace.
#[derive(Debug, PartialEq)]
pub struct OctoTextRef {
    /// Retain workspace reference to improve troubleshooting if someone tries to use an [OctoBlockRef]
    /// from one [OctoWorkspaceRef] in a different [OctoWorkspaceRef].
    workspace_ref: OctoWorkspaceRef,
    /// Internal reference directly to the [yrs::MapRef] in the "Workspace"
    yrs_ref: yrs::TextRef,
}

// pub struct OctoWorkspaceContent {
//     /// Internal: Do not let anyone have this without requiring mutable access to [OctoWorkspaceContent].
//     doc: yrs::Doc,
//     octo_ref: OctoWorkspaceRef,
// }

pub struct OctoWorkspace {
    /// Internal: Do not let anyone have this without requiring mutable access to [OctoWorkspaceContent].
    doc: yrs::Doc,
    octo_ref: OctoWorkspaceRef,
    // /// Separated data type to be surfaced to workspace plugins.
    // content: OctoWorkspaceContent,
    /// Plugins which extend the workspace through subscriptions such as for text search.
    ///
    /// TODO [crate::workspace::plugins::setup_plugin]
    plugins: plugins::PluginMap,
}

// mark non exhaustive so people must use the `new` constructor
#[non_exhaustive]
pub struct OctoBlockCreateOptions {
    /// ID to use for the creation of this block. On conflicting ID, you might see a
    /// [OctoCreateError::IDConflict] error upon attempted creation.
    pub id: Rc<str>,
    /// Initial [concepts::SysPropFlavor]s for this block.
    pub flavor: String,
    /// Initial [concepts::UserDefinedProp]s for this block.
    /// Future: Consider wrapping our own [lib0::any::Any] type.
    /// These names will be prefixed by `prop:${key}` before being inserted, so do not supply your own `prop:` prefix.
    pub properties: HashMap<String, lib0::any::Any>,
}

impl OctoBlockCreateOptions {
    pub fn new(id: Rc<str>, flavor: String, properties: HashMap<String, lib0::any::Any>) -> Self {
        Self {
            id,
            flavor,
            properties,
        }
    }
}

pub struct OctoWriter<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn_mut: TransactionMut<'doc>,
}

pub struct OctoReader<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn: Transaction<'doc>,
}

// impl<'doc> ReadTxn for OctoReader<'doc> {
//     fn store(&self) -> &yrs::Store {
//         self.yrs_txn.store()
//     }
// }

// impl<'doc> ReadTxn for OctoWriter<'doc> {
//     fn store(&self) -> &yrs::Store {
//         self.yrs_txn_mut.store()
//     }
// }

pub trait OctoRead<'doc> {
    type YRSReadTxn: ReadTxn;
    fn yrs_parts(&self) -> (&OctoWorkspaceRef, &Self::YRSReadTxn);
    fn yrs_read_txn(&self) -> &Self::YRSReadTxn {
        &self.yrs_parts().1
    }

    /// Get an [OctoBlockRef] for the given `block_id`.
    fn get_block(&self, block_id: &str) -> Option<OctoBlockRef> {
        let (workspace_ref, yrs_txn) = self.yrs_parts();
        let OctoWorkspaceRef {
            ref yrs_block_ref,
            ref yrs_updated_ref,
            ..
        } = workspace_ref;

        match (yrs_block_ref.get(yrs_txn, block_id), yrs_updated_ref.get(yrs_txn, block_id)) {
            (Some(block_map), Some(history_array)) => {
                let yrs_props_ref = block_map.to_ymap().expect("block props is ymap");
                let yrs_history_ref = history_array.to_yarray().expect("block updated is yarray");
                Some(OctoBlockRef {
                    id: block_id.to_owned().into(),
                    workspace_ref: workspace_ref.clone(),
                    yrs_history_ref,
                    yrs_props_ref,
                })
            }
            (None, None) => None,
            unexpected => unreachable!(
                "expected block({block_id:?}) to have both a props map and history array or neither, but found: {unexpected:?}"
            ),
        }
    }

    fn contains_block(&self, block_id: &str) -> bool {
        let (workspace_ref, yrs_txn) = self.yrs_parts();
        let OctoWorkspaceRef {
            ref yrs_block_ref,
            ref yrs_updated_ref,
            ..
        } = workspace_ref;

        match (yrs_block_ref.contains(yrs_txn, &block_id), yrs_updated_ref.contains(yrs_txn, &block_id)) {
            (true, true) => true,
            (false, false) => false,
            unexpected => unreachable!(
                "expected block({block_id:?}) to have both a props map and history array or neither, but found: {unexpected:?}"
            ),
        }
    }
}

// impl<'doc, W: OctoWrite> OctoRead<'doc> for W {
//     type YRSReadTxn = TransactionMut<'doc>;

//     fn yrs_parts(&self) -> (&OctoWorkspaceRef, &Self::YRSReadTxn);
// }

pub trait OctoWrite<'doc>: OctoRead<'doc> {
    fn yrs_parts_mut(&mut self) -> (&OctoWorkspaceRef, &mut TransactionMut);

    /// Create a block, returning an [OctoBlockRef] or erroring with [OctoCreateError].
    fn create_block<T: Into<OctoBlockCreateOptions>>(
        &mut self,
        options: T,
    ) -> Result<OctoBlockRef, OctoCreateError> {
        let OctoBlockCreateOptions {
            id: new_block_id,
            properties,
            flavor,
        } = options.into();

        if self.contains_block(&new_block_id) {
            return Err(OctoCreateError::IDConflict { id: new_block_id });
        }

        let (workspace_ref, yrs_txn_mut) = self.yrs_parts_mut();
        let OctoWorkspaceRef {
            ref yrs_block_ref,
            ref yrs_updated_ref,
            ..
        } = workspace_ref;

        let (props_ref, history_ref) = {
            // should not panic because we required exclusive borrow of self
            let preexisting_values = (
                yrs_block_ref.insert(yrs_txn_mut, new_block_id.clone(), MapPrelim::<Any>::new()),
                yrs_updated_ref.insert(
                    yrs_txn_mut,
                    new_block_id.clone(),
                    ArrayPrelim::<Vec<String>, String>::from(vec![]),
                ),
            );

            // is checking this overkill?
            #[cfg(debug_assertions)]
            if preexisting_values.0.is_some() || preexisting_values.1.is_some() {
                unreachable!("Should have been an ID Conflict for {new_block_id:?}. Found values already existing for block: {preexisting_values:?}");
            }

            (
                yrs_block_ref
                    .get(yrs_txn_mut, &new_block_id)
                    .and_then(|b| b.to_ymap())
                    .expect("new block props is map"),
                yrs_updated_ref
                    .get(yrs_txn_mut, &new_block_id)
                    .and_then(|b| b.to_yarray())
                    .expect("new block updated is array"),
            )
        };

        // See https://doc.rust-lang.org/rust-by-example/macros/repeat.html
        macro_rules! insert_prop_and_prelim_pairs {
            ($(($id:expr, $val:expr)),+) => { $(props_ref.insert(yrs_txn_mut, $id, $val);)+ }
        }

        // init default schema
        insert_prop_and_prelim_pairs!(
            ("sys:flavor", flavor.as_ref()),
            ("sys:version", ArrayPrelim::from([1, 0])),
            (
                "sys:children",
                ArrayPrelim::<Vec<String>, String>::from(vec![])
            ),
            ("sys:created", chrono::Utc::now().timestamp_millis() as f64)
        );

        // insert initial properties from options
        for (key, value) in properties {
            props_ref.insert(yrs_txn_mut, format!("prop:{key}"), value);
        }

        Ok(OctoBlockRef {
            id: new_block_id,
            workspace_ref: workspace_ref.clone(),
            yrs_props_ref: props_ref,
            yrs_history_ref: history_ref,
        })
    }
}

impl OctoWorkspace {
    pub fn get_ref(&self) -> &OctoWorkspaceRef {
        &self.octo_ref
    }

    // /// Progress 0/10:
    // ///  * Do we actually need this extra "content" thing?
    // ///  * Or can we just do everything through a trasaction?
    // pub fn get_content<T: Into<OctoBlockCreateOptions>>(&self) -> &OctoWorkspaceContent {
    //     &self.content
    // }
}

/// Key for [concepts::SysPropFlavor].
const PROPS_FLAVOR_KEY: &str = "sys:flavor";

impl OctoBlockRef {
    /// Question: If "blocka" is created, deleted, and re-created, should the first BlockRef still be valid?
    fn check_exists<R: ReadTxn>(
        &self,
        workspace_ref: &OctoWorkspaceRef,
        yrs_read_txn: &R,
    ) -> Result<(), OctoError> {
        // Is there a "fastest" way to determine if a MapRef is still a part of a document?

        // false for formatting
        if false
            || workspace_ref
                .yrs_block_ref
                .get(yrs_read_txn, &self.id)
                .and_then(|value| value.to_ymap())
                .is_none()
            || workspace_ref
                .yrs_updated_ref
                .get(yrs_read_txn, &self.id)
                .and_then(|value| value.to_yarray())
                .is_none()
        {
            return Err(OctoError::BlockNotFound {
                id: self.id.clone(),
            });
        }

        Ok(())
    }

    /// Internal direct access
    fn try_yrs_prop_value<'doc, R, T, F>(
        &self,
        reader: &R,
        prop_key: &str,
        map_fn: F,
    ) -> Result<T, OctoError>
    where
        R: OctoRead<'doc>,
        F: FnOnce(yrs::types::Value, &R::YRSReadTxn) -> Result<T, OctoError>,
    {
        let (workspace_ref, yrs_read_txn) = reader.yrs_parts();
        self.check_txn(workspace_ref)?;
        // If a block was deleted, should we still return its flavor if we have it?
        self.check_exists(workspace_ref, yrs_read_txn)?;
        self.yrs_props_ref
            .get(yrs_read_txn, prop_key)
            .ok_or_else(|| OctoError::BlockPropertyNotFound {
                id: self.id.clone(),
                property_key: prop_key.into(),
            })
            .and_then(|value| map_fn(value, yrs_read_txn))
    }

    /// See [concepts::SysPropFlavor].
    pub fn get_flavor<'doc, R: OctoRead<'doc>>(&self, reader: &R) -> Result<String, OctoError> {
        self.try_yrs_prop_value(reader, PROPS_FLAVOR_KEY, |value, yrs_read_txn| {
            use yrs::types::Value;
            match value {
                Value::Any(lib0::any::Any::String(value)) => Ok(value.to_string()),
                other => Err(OctoError::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    property_key: PROPS_FLAVOR_KEY.into(),
                    expected_type: "Any::String",
                    found: other.to_string(yrs_read_txn),
                }),
            }
        })
    }

    /// See [concepts::YJSBlockHistory].
    pub fn get_history<'doc, R: OctoRead<'doc>>(
        &self,
        reader: &R,
    ) -> Result<Cow<'doc, str>, OctoError> {
        let (workspace_ref, yrs_read_txn) = reader.yrs_parts();
        self.check_txn(workspace_ref)?;
        self.check_exists(workspace_ref, yrs_read_txn)?;
        Ok(self
            .yrs_props_ref
            .get(yrs_read_txn, PROPS_FLAVOR_KEY)
            .map(|prop| prop.to_string(yrs_read_txn))
            .expect("has sys:flavor prop")
            .into())
    }

    /// Inserts a new `value` under given `key` into current map. Returns a value stored previously
    /// under the same key (if any existed).
    pub fn insert(
        &self,
        writer: &mut OctoWriter,
        key: &str,
        value: lib0::any::Any,
    ) -> Result<Option<yrs::types::Value>, OctoCreateError> {
        self.check_txn(&*writer)?;
        Ok(self
            .yrs_props_ref
            .insert(&mut writer.yrs_txn_mut, key, value))
    }

    pub fn id(&self) -> &str {
        self.id.as_ref()
    }
}

#[derive(Error, Debug)]
pub enum OctoError {
    /// Sometimes, you might have a block created from a different workspace
    #[error("block with id `{id:?}` not found")]
    BlockNotFound { id: Rc<str> },
    /// Sometimes, you might have a block created from a different workspace
    #[error("block `{id:?}` property `{property_key:?}` not found")]
    BlockPropertyNotFound { id: Rc<str>, property_key: Rc<str> },
    /// Sometimes, you might have a block created from a different workspace
    #[error("block with id `{id:?}` and property `{property_key}` expected `{expected_type}` but found `{found}`")]
    BlockPropertyWasUnexpectedType {
        id: Rc<str>,
        property_key: Rc<str>,
        expected_type: &'static str,
        found: String,
    },
    /// Tried to use a Transaction associated with a different Workspace
    #[error("transaction was associated with a different workspace than the item")]
    TransactionFromDifferentWorkspace,
}

/// Private error type tag that gets mapped into other appropriate Octo errors like [OctoCreateError::TransactionFromDifferentWorkspace]
struct DifferentWorkspaces;
/// private convenience fn see [DifferentWorkspaces] error type.
trait AssociatedWorkspace {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef;
    fn check_txn<Other: AssociatedWorkspace>(
        &self,
        other: &Other,
    ) -> Result<(), DifferentWorkspaces> {
        if self.get_associated_workspace().id != other.get_associated_workspace().id {
            Err(DifferentWorkspaces)
        } else {
            Ok(())
        }
    }
}

impl AssociatedWorkspace for OctoBlockRef {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef {
        &self.workspace_ref
    }
}
impl AssociatedWorkspace for OctoWorkspaceRef {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef {
        &self
    }
}
impl<'doc> AssociatedWorkspace for OctoWriter<'doc> {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef {
        &self.workspace_ref
    }
}
impl<'doc> AssociatedWorkspace for OctoReader<'doc> {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef {
        &self.workspace_ref
    }
}

impl From<DifferentWorkspaces> for OctoError {
    fn from(_: DifferentWorkspaces) -> Self {
        Self::TransactionFromDifferentWorkspace
    }
}

#[derive(Error, Debug)]
pub enum OctoCreateError {
    /// Something with this ID already exists
    #[error("item with id `{id:?}` already exists")]
    IDConflict { id: Rc<str> },
    /// Tried to use a Transaction associated with a different Workspace
    #[error("transaction was associated with a different workspace than the item")]
    #[from(DifferentWorkspaces)]
    TransactionFromDifferentWorkspace,
}

impl From<DifferentWorkspaces> for OctoCreateError {
    fn from(_: DifferentWorkspaces) -> Self {
        Self::TransactionFromDifferentWorkspace
    }
}
