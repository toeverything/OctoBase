//! Next iteration of the OctoBase design.
//! Progress 0/10:
//!
//! This is a clean rewrite of OctoBase concepts following the upgrade
//! of yrs 0.14 to support subdocs.
//!
//! At this moment, everything is in one file to create a pressure not to
//! over-complicate or over-expand the surface area of OctoBase core.
// use crate::prelude::*;

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

mod plugins;

pub use plugins::{OctoPlugin, OctoPluginRegister, OctoPluginUpdateError};
use std::{borrow::Cow, collections::HashMap, fmt::Debug, sync::Arc};
use thiserror::Error;
use yrs::{Map, ReadTxn, Transact, Transaction, TransactionMut};

/// See [OctoWorkspace] and [OctoWorkspaceContent].
#[derive(Clone, Debug, PartialEq)]
pub struct OctoWorkspaceRef {
    id: Arc<str>,
    // Perhaps we should include a way to
    // identify the operator / client id
    // via the Workspace's Awareness
    /// Indexed by "block id", the values are the corresponding block's [concepts::YJSBlockPropMap]s.
    yrs_block_ref: yrs::MapRef,
    /// Indexed by "block id", the values are the corresponding block's [concepts::YJSBlockHistory].
    yrs_updated_ref: yrs::MapRef,
    /// Used for assorted attributes of the workspace like the Workspace's icon or display name.
    yrs_metadata_ref: yrs::MapRef,
}

/// A reference to a Block in a Workspace.
#[derive(Debug, PartialEq)]
pub struct OctoBlockRef {
    /// Unique Block ID
    id: Arc<str>,
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

// /// Progress 0/10:
// ///  * Do we actually need this extra "content" thing?
// ///  * Or can we just do everything through a transaction?
// pub struct OctoWorkspaceContent {
//     /// Internal: Do not let anyone have this.
//     /// Only allow interaction with this when with mutable access to [OctoWorkspaceContent].
//     yrs_doc_ref: yrs::Doc,
//
//     /// Workspace block references.
//     octo_ref: OctoWorkspaceRef,
//
//     /// a simple shared state protocol that can be used for non-persistent data like awareness information
//     /// (cursor, username, status, ..). Each client can update its own local state and listen to state
//     /// changes of remote clients.
//     y_sync_awareness: Awareness,
// }

pub struct OctoWorkspace {
    /// Internal: Do not let anyone have this without requiring mutable access to [OctoWorkspaceContent].
    yrs_doc_ref: yrs::Doc,

    /// Workspace block references.
    octo_ref: OctoWorkspaceRef,

    // Does this need to be wrapped into an "OctoWorkspaceContent" struct?
    // I guess this question is answered by whether it makes sense for plugins
    // to interact with awareness?
    /// A simple shared state protocol that can be used for non-persistent data like awareness information
    /// (cursor, username, status, ..). Each client can update its own local state and listen to state
    /// changes of remote clients.
    y_sync_awareness: y_sync::awareness::Awareness,

    // May not be necessary with OctoRead / OctoWrite
    // /// Separated data type to be surfaced to workspace plugins.
    // content: OctoWorkspaceContent,
    /// Plugins which extend the workspace through subscriptions such as for text search.
    ///
    /// We store plugins so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    ///
    /// See [plugins].
    plugins: plugins::PluginMap,
}

// hmm... PluginMap is not send
unsafe impl Send for OctoWorkspace {}
unsafe impl Sync for OctoWorkspace {}

impl OctoWorkspace {
    pub fn from_doc<S: AsRef<str>>(doc: yrs::Doc, id: S) -> OctoWorkspace {
        // TODO: Initial index in Tantivy including:
        //  * Tree visitor collecting all child blocks which are correct flavor for extracted text
        //  * Extract prop:text / prop:title for index to block ID in Tantivy
        let yrs_block_ref = doc.get_or_insert_map("blocks");
        let yrs_updated_ref = doc.get_or_insert_map("updated");
        let yrs_metadata_ref = doc.get_or_insert_map("space:meta");
        let octo_ref = OctoWorkspaceRef {
            id: id.as_ref().into(),
            yrs_block_ref,
            yrs_metadata_ref,
            yrs_updated_ref,
        };

        Self {
            y_sync_awareness: y_sync::awareness::Awareness::new(doc.clone()),
            octo_ref,
            yrs_doc_ref: doc,
            plugins: Default::default(),
        }
    }

    // TODO: Separate this into a non-core jwst crate
    // workspace
    //     .register_builtin_plugins()
    //     // FUTURE: Consider other kinds of errors that could be Tantivy specific
    //     .expect("no errors registering built-in plugins on a clean workspace");
    // // I want to deprecate this in favor of implementors explicitly loading their own
    // // plugins by choice.
    // pub fn register_default_plugins(
    //     &mut self,
    // ) -> Result<&mut OctoWorkspace, OctoPluginRegistrationError> {
    //     // default plugins
    //     if cfg!(feature = "workspace-search") {
    //         // Set up text search plugin
    //         insert_plugin(
    //             workspace,
    //             octo_text_search::TextSearchPluginConfig::default(),
    //         )?;
    //     }
    //     Ok(workspace)
    // }

    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    pub fn update_plugin<P: OctoPlugin>(&mut self) -> Result<(), OctoPluginUpdateError> {
        let read = OctoReader {
            workspace_ref: self.octo_ref.clone(),
            yrs_txn: self.yrs_doc_ref.transact(),
        };
        self.plugins.update_plugin::<P>(&read)
    }

    /// Create an [OctoReader] which implements [OctoRead].
    pub fn read(&self) -> OctoReader {
        OctoReader {
            workspace_ref: self.octo_ref.clone(),
            yrs_txn: self.yrs_doc_ref.transact(),
        }
    }

    /// Create an [OctoWriter] which implements [OctoWrite].
    pub fn write(&mut self) -> OctoWriter {
        OctoWriter {
            workspace_ref: self.octo_ref.clone(),
            yrs_txn_mut: self.yrs_doc_ref.transact_mut(),
        }
    }

    /// See [plugins].
    pub fn get_plugin<P: OctoPlugin>(&self) -> Option<&P> {
        self.plugins.get_plugin::<P>()
    }

    /// Setup a [plugins::OctoPlugin] and insert it into the [Workspace].
    /// See [plugins].
    pub fn register_plugin(
        &mut self,
        config: impl plugins::OctoPluginRegister,
    ) -> Result<&mut OctoWorkspace, OctoPluginRegistrationError> {
        let plugin = config.setup(self)?;
        self.plugins
            .insert_plugin(plugin)
            .map_err(|err| match err {
                plugins::PluginInsertError::PluginConflict => {
                    OctoPluginRegistrationError::PluginConflict
                }
            })?;

        Ok(self)
    }

    /// Subscribe to update events.
    pub fn observe(
        &mut self,
        f: impl Fn(&OctoReaderForEvent, &OctoUpdateEvent) -> () + 'static,
    ) -> Option<OctoSubscription> {
        let workspace_ref = self.get_ref().clone();
        // TODO: should we be able to expect able to observe?
        // OR: Should we return a Result...
        self.y_sync_awareness
            .doc_mut()
            .observe_update_v1(move |txn, update_event| {
                // todo: do something with txn
                f(
                    &OctoReaderForEvent(OctoReaderForEventInner(txn), &workspace_ref),
                    &OctoUpdateEvent(update_event),
                )
            })
            .map(OctoSubscription)
            .ok()
    }
}

/// Drop this subscription to unsubscribe.
pub struct OctoSubscription(yrs::UpdateSubscription);

/// Solves a problem where we need the lifetime tied to self of [OctoReaderForEvent]
/// in order to implement [yrs::ReadTxn] without lifetime issues.
pub struct OctoReaderForEventInner<'update>(&'update TransactionMut<'update>);
impl<'update> yrs::ReadTxn for OctoReaderForEventInner<'update> {
    fn store(&self) -> &yrs::Store {
        &self.0.store()
    }
}
pub struct OctoReaderForEvent<'update>(OctoReaderForEventInner<'update>, &'update OctoWorkspaceRef);
impl<'update> OctoRead<'update> for OctoReaderForEvent<'update> {
    type YRSReadTxn = OctoReaderForEventInner<'update>;

    fn _parts(&self) -> _ReadParts<Self::YRSReadTxn> {
        _ReadParts(&self.1, &self.0)
    }
}

/// TODO: Expose useful ways to use this event's info, like
/// listing the block ids that were updated.
pub struct OctoUpdateEvent<'update>(&'update yrs::UpdateEvent);

/// Error from [OctoWorkspace::register_plugin].
#[derive(Error, Debug)]
pub enum OctoPluginRegistrationError {
    #[error("plugin of type already exists")]
    PluginConflict,
    #[error("plugin update() returned error")]
    UpdateError(#[from] Box<dyn std::error::Error>),
}

/// Value assigned to a block or metadata
pub struct OctoValue(yrs::types::Value);

impl OctoValue {
    pub fn try_as_string(&self) -> Result<String, OctoValueError> {
        use lib0::any::Any;
        use yrs::types::Value;
        match &self.0 {
            Value::Any(Any::String(box_str)) => return Ok(box_str.to_string()),
            _ => Err(OctoValueError::WrongType {
                actual: self.actual_type(),
                expected: "basic string",
            }),
        }
    }
    // this is for constructing error only, not for stabilizing
    fn actual_type(&self) -> &'static str {
        use lib0::any::Any;
        use yrs::types::Value;
        match &self.0 {
            Value::Any(any_value) => match any_value {
                Any::Null => "basic null",
                Any::Undefined => "basic undefined",
                Any::Bool(_) => "basic boolean",
                Any::Number(_) => "basic number",
                Any::BigInt(_) => "basic bigint",
                Any::String(_) => "basic string",
                Any::Buffer(_) => "basic buffer",
                Any::Array(_) => "basic array",
                Any::Map(_) => "basic map",
            },
            Value::YText(_) => "octo Text",
            Value::YArray(_) => "octo Array",
            Value::YMap(_) => "octo Map",
            Value::YXmlElement(_) => "octo Xml Element",
            Value::YXmlFragment(_) => "octo Xml Fragment",
            Value::YXmlText(_) => "octo Xml Text",
            Value::YDoc(_) => "octo Doc",
        }
    }
}

#[derive(Debug, Error)]
pub enum OctoValueError {
    #[error("value of type `{actual}` was a different type than expected `{expected}`")]
    WrongType {
        actual: &'static str,
        expected: &'static str,
    },
}

impl From<yrs::types::Value> for OctoValue {
    fn from(yrs_value: yrs::types::Value) -> Self {
        OctoValue(yrs_value)
    }
}

// mark non exhaustive so people must use the `new` constructor
#[non_exhaustive]
pub struct OctoBlockCreateOptions {
    /// ID to use for the creation of this block. On conflicting ID, you might see a
    /// [OctoCreateError::IDConflict] error upon attempted creation.
    pub id: Arc<str>,
    /// Initial [concepts::SysPropFlavor]s for this block.
    pub flavor: String,
    /// Initial [concepts::UserDefinedProp]s for this block.
    /// Future: Consider wrapping our own [lib0::any::Any] type.
    /// These names will be prefixed by `prop:${key}` before being inserted, so do not supply your own `prop:` prefix.
    pub properties: HashMap<String, lib0::any::Any>,
}

impl OctoBlockCreateOptions {
    pub fn new(id: Arc<str>, flavor: String, properties: HashMap<String, lib0::any::Any>) -> Self {
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

impl<'doc> OctoRead<'doc> for OctoWriter<'doc> {
    type YRSReadTxn = TransactionMut<'doc>;

    fn _parts(&self) -> _ReadParts<'_, Self::YRSReadTxn> {
        _ReadParts(&self.workspace_ref, &self.yrs_txn_mut)
    }
}

impl<'doc> OctoWrite<'doc> for OctoWriter<'doc> {
    fn _parts_mut(&mut self) -> _WriteParts<'_, TransactionMut<'doc>> {
        _WriteParts(&self.workspace_ref, &mut self.yrs_txn_mut)
    }
}

pub struct OctoReader<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn: Transaction<'doc>,
}

impl<'doc> OctoRead<'doc> for OctoReader<'doc> {
    type YRSReadTxn = Transaction<'doc>;

    fn _parts(&self) -> _ReadParts<'_, Transaction<'doc>> {
        _ReadParts(&self.workspace_ref, &self.yrs_txn)
    }
}
/// Create a private type like this so it's not possible for another crate to implement [OctoRead],
/// and prevent another crate from accessing underlying crate properties directly.
pub struct _ReadParts<'a, T>(&'a OctoWorkspaceRef, &'a T);

/// Create a private type like this so it's not possible for another crate to implement [OctoWrite].
pub struct _WriteParts<'a, T>(&'a OctoWorkspaceRef, &'a mut T);

fn user_prop_key(unprefixed: &str) -> String {
    format!("prop:{unprefixed}")
}

pub trait OctoRead<'doc> {
    type YRSReadTxn: yrs::ReadTxn;
    #[doc(hidden)]
    fn _parts(&self) -> _ReadParts<Self::YRSReadTxn>;

    /// Get an [OctoBlockRef] for the given `block_id`.
    fn get_block(&self, block_id: &str) -> Option<OctoBlockRef> {
        let _ReadParts(workspace_ref, yrs_txn) = self._parts();
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

    /// TOO: Rename to `all_blocks`
    // Why inline? Should just default to LLVM's inlining choices...
    #[inline]
    fn block_iter<'a>(&'a self) -> Box<dyn Iterator<Item = OctoBlockRef> + 'a> {
        let _ReadParts(workspace_ref, yrs_txn) = self._parts();
        let OctoWorkspaceRef {
            ref yrs_block_ref,
            ref yrs_updated_ref,
            ..
        } = workspace_ref;

        // Question: Why does block_iter care about updated, while block_count doesn't?
        let iter = yrs_block_ref
            .iter(yrs_txn)
            .zip(yrs_updated_ref.iter(yrs_txn))
            // depends on the list of blocks and the list of updated to be always in sync, always in same order, and always same length...
            // that kinda smells like needing a special container is necessary.
            .map(|((id, block), (_, updated))| OctoBlockRef {
                id: id.to_string().into(),
                workspace_ref: workspace_ref.clone(),
                yrs_history_ref: updated.to_yarray().expect("block updated"),
                yrs_props_ref: block.to_ymap().expect("block props"),
            })
            // Originally, this was collected and into_iter to save time during
            // yrs 0.14 update.
            //
            // TODO: consider if we can actually make this an iterator, or
            // if there is a long term issue with holding a Transaction open
            // while iterating, because it read locks the document.
            .collect::<Vec<_>>()
            .into_iter();

        Box::new(iter)
    }

    fn contains_block(&self, block_id: &str) -> bool {
        let _ReadParts(workspace_ref, yrs_txn) = self._parts();
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

    fn get_workspace_metadata(&self, metadata_key: &str) -> Option<OctoValue> {
        let _ReadParts(workspace_ref, yrs_txn) = self._parts();
        let OctoWorkspaceRef {
            ref yrs_metadata_ref,
            ..
        } = workspace_ref;

        yrs_metadata_ref.get(yrs_txn, metadata_key).map(Into::into)
    }
}

pub trait OctoWrite<'doc>: OctoRead<'doc> {
    fn _parts_mut(&mut self) -> _WriteParts<'_, TransactionMut<'doc>>;

    /// Create a block, returning an [OctoBlockRef] or erroring with [OctoCreateError].
    fn create_block<T: Into<OctoBlockCreateOptions>>(
        &'doc mut self,
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

        let _WriteParts(workspace_ref, yrs_txn_mut) = self._parts_mut();
        let OctoWorkspaceRef {
            ref yrs_block_ref,
            ref yrs_updated_ref,
            ..
        } = workspace_ref;

        let (props_ref, history_ref) = {
            // should not panic because we required exclusive borrow of self
            let preexisting_values = (
                yrs_block_ref.insert(
                    yrs_txn_mut,
                    new_block_id.as_ref(),
                    yrs::MapPrelim::<lib0::any::Any>::new(),
                ),
                yrs_updated_ref.insert(
                    yrs_txn_mut,
                    new_block_id.as_ref(),
                    yrs::ArrayPrelim::<Vec<String>, String>::from(vec![]),
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
            (SYS_FLAVOR_KEY, flavor.as_ref()),
            (SYS_VERSION_KEY, yrs::ArrayPrelim::from([1, 0])),
            (
                SYS_CHILDREN_KEY,
                yrs::ArrayPrelim::<Vec<String>, String>::from(vec![])
            ),
            (
                SYS_CREATED_KEY,
                chrono::Utc::now().timestamp_millis() as f64
            )
        );

        // insert initial properties from options
        for (key, value) in properties {
            props_ref.insert(yrs_txn_mut, user_prop_key(&key), value);
        }

        Ok(OctoBlockRef {
            id: new_block_id.into(),
            workspace_ref: workspace_ref.clone(),
            yrs_props_ref: props_ref,
            yrs_history_ref: history_ref,
        })
    }

    // /// Set user attributes for a block. See [concepts::UserDefinedProp].
    // ///
    // /// Internal: In the raw JSON, these properties will come out with a prefix of `"prop:"`.
    // /// So, if you set
    // fn set_props<K, P>(&mut self, block_ref: OctoBlockRef, properties: P)
    // where
    //     K: AsRef<str>,
    //     P: IntoIterator<Item = (K, lib0::any::Any)>,
    // {
    //     let (_, yrs_txn_mut) = self.yrs_parts_mut();
    //     // insert initial properties from options
    //     for (key, value) in properties {
    //         block_ref
    //             .yrs_props_ref
    //             .insert(yrs_txn_mut, format!("prop:{}", key.as_ref()), value);
    //     }
    // }
}

// mod octo_write_tests {
//     use crate::OctoWrite;

//     use super::OctoBlockCreateOptions;

//     pub fn test_set_props<W: OctoWrite>(writer: W) {
//         let block_1 = writer
//             .create_block(OctoBlockCreateOptions {
//                 id: "abc".into(),
//                 flavor: "any-flavor-1".to_string(),
//                 properties: Default::default(),
//             })
//             .expect("created block");
//         block_1.get_flavor(reader)
//     }
// }

impl OctoWorkspace {
    pub fn get_ref(&self) -> &OctoWorkspaceRef {
        &self.octo_ref
    }

    // /// Progress 0/10:
    // ///  * Do we actually need this extra "content" thing?
    // ///  * Or can we just do everything through a transaction?
    // pub fn get_content<T: Into<OctoBlockCreateOptions>>(&self) -> &OctoWorkspaceContent {
    //     &self.content
    // }
}

/// [concepts::SysProp] key for [concepts::SysPropFlavor].
const SYS_FLAVOR_KEY: &str = "sys:flavor";
/// [concepts::SysProp] key for children.
const SYS_CHILDREN_KEY: &str = "sys:children";
/// [concepts::SysProp] key for version.
const SYS_VERSION_KEY: &str = "sys:version";
/// [concepts::SysProp] key for created time.
const SYS_CREATED_KEY: &str = "sys:created";

impl OctoBlockRef {
    /// Question: If "blocka" is created, deleted, and re-created, should the first BlockRef still be valid?
    fn check_exists<R: yrs::ReadTxn>(
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
        let _ReadParts(workspace_ref, yrs_read_txn) = reader._parts();
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
        self.try_yrs_prop_value(reader, SYS_FLAVOR_KEY, |value, yrs_read_txn| {
            use yrs::types::Value;
            match value {
                Value::Any(lib0::any::Any::String(value)) => Ok(value.to_string()),
                other => Err(OctoError::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    property_key: SYS_FLAVOR_KEY.into(),
                    // TODO: be more consistent with these type names, yet.
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
        let _ReadParts(workspace_ref, yrs_read_txn) = reader._parts();
        self.check_txn(workspace_ref)?;
        self.check_exists(workspace_ref, yrs_read_txn)?;
        Ok(self
            .yrs_props_ref
            .get(yrs_read_txn, SYS_FLAVOR_KEY)
            .map(|prop| prop.to_string(yrs_read_txn))
            .expect("has sys:flavor prop")
            .into())
    }

    /// Set a `value` under given user defined `key` into current map.
    /// Returns a value stored previously under the same key (if any existed).
    /// See [concepts::UserDefinedProp].
    pub fn set_user_prop(
        &self,
        writer: &mut OctoWriter,
        key: &str,
        value: lib0::any::Any,
    ) -> Result<Option<yrs::types::Value>, OctoEditError> {
        self.check_txn(&*writer)?;
        Ok(self
            .yrs_props_ref
            .insert(&mut writer.yrs_txn_mut, format!("prop:{key}"), value))
    }

    // Is raw set needed?
    // /// Inserts a new `value` under given `key` into current map. Returns a value stored previously
    // /// under the same key (if any existed).
    // fn set_prop_raw(
    //     &self,
    //     writer: &mut OctoWriter,
    //     key: &str,
    //     value: lib0::any::Any,
    // ) -> Result<Option<yrs::types::Value>, OctoEditError> {
    //     self.check_txn(&*writer)?;
    //     Ok(self
    //         .yrs_props_ref
    //         .insert(&mut writer.yrs_txn_mut, key, value))
    // }

    pub fn id(&self) -> &str {
        self.id.as_ref()
    }
}

#[derive(Error, Debug)]
pub enum OctoError {
    /// Sometimes, you might have a block created from a different workspace
    #[error("block with id `{id:?}` not found")]
    BlockNotFound { id: Arc<str> },
    /// Sometimes, you might have a block created from a different workspace
    #[error("block `{id:?}` property `{property_key:?}` not found")]
    BlockPropertyNotFound {
        id: Arc<str>,
        property_key: Arc<str>,
    },
    /// Sometimes, you might have a block created from a different workspace
    #[error("block with id `{id:?}` and property `{property_key}` expected `{expected_type}` but found `{found}`")]
    BlockPropertyWasUnexpectedType {
        id: Arc<str>,
        property_key: Arc<str>,
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
    IDConflict { id: Arc<str> },
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

#[derive(Error, Debug)]
pub enum OctoEditError {
    /// Tried to use a Transaction associated with a different Workspace
    #[error("transaction was associated with a different workspace than the item")]
    #[from(DifferentWorkspaces)]
    TransactionFromDifferentWorkspace,
}

impl From<DifferentWorkspaces> for OctoEditError {
    fn from(_: DifferentWorkspaces) -> Self {
        Self::TransactionFromDifferentWorkspace
    }
}
