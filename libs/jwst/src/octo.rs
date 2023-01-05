//! Next iteration of the OctoBase design.
//! Progress 1/10:
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
mod yjs_binary;

pub use plugins::{OctoPlugin, OctoPluginRegister, OctoPluginUpdateError};
use std::{borrow::Cow, collections::HashMap, fmt::Debug, sync::Arc};
use thiserror::Error;
use yrs::{Array, Map, Transact, Transaction, TransactionMut};

use crate::{utils::JS_INT_RANGE, BlockHistory, HistoryOperation, OctoResult};

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

/// A local copy of a workspace.
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

/// Builder for [OctoWorkspace].
pub struct OctoWorkspaceBuilder {
    id: String,
    doc_options: yrs::Options,
}

impl OctoWorkspaceBuilder {
    pub fn skip_gc(mut self, val: bool) -> Self {
        self.doc_options.skip_gc = val;
        self
    }

    pub fn build_empty(self) -> OctoWorkspace {
        let OctoWorkspaceBuilder { id, doc_options } = self;
        let doc = yrs::Doc::with_options(doc_options);
        OctoWorkspace::from_doc(doc, id)
    }

    /// For internal tests from an existing [yrs::Doc].
    pub(crate) fn build_with_yrs_doc<F>(self, make_doc_fn: F) -> OctoWorkspace
    where
        F: FnOnce(yrs::Doc) -> yrs::Doc,
    {
        let OctoWorkspaceBuilder { id, doc_options } = self;
        OctoWorkspace::from_doc(make_doc_fn(yrs::Doc::with_options(doc_options)), id)
    }
}

impl OctoWorkspace {
    pub fn builder(id: impl AsRef<str>) -> OctoWorkspaceBuilder {
        OctoWorkspaceBuilder {
            id: id.as_ref().to_string(),
            doc_options: Default::default(),
        }
    }

    pub fn migrate(&mut self) -> yjs_binary::OctoWorkspaceMigrator<'_> {
        yjs_binary::OctoWorkspaceMigrator {
            yrs_txn_mut: self.yrs_doc_ref.transact_mut(),
        }
    }

    pub fn export(&self) -> yjs_binary::OctoWorkspaceExporter<'_> {
        yjs_binary::OctoWorkspaceExporter {
            yrs_txn: self.yrs_doc_ref.transact(),
        }
    }

    /// Private, as we should not be enabling our dependents to use [yrs::Doc] directly.
    pub(crate) fn from_doc<S: AsRef<str>>(doc: yrs::Doc, id: S) -> OctoWorkspace {
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
            doc_client_id: self.yrs_doc_ref.client_id(),
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
/// Usually, this comes from a change observation made on "observe()" on [OctoWorkspace].
pub struct OctoSubscription(yrs::UpdateSubscription);

/// Solves a problem where we need the lifetime tied to self of [OctoReaderForEvent]
/// in order to implement [yrs::ReadTxn] without lifetime issues.
pub struct OctoReaderForEventInner<'update>(&'update TransactionMut<'update>);
impl<'update> yrs::ReadTxn for OctoReaderForEventInner<'update> {
    fn store(&self) -> &yrs::Store {
        &self.0.store()
    }
}

/// Implements [OctoRead] for reading refs out of an [OctoWorkspace].
/// Usually, this comes from a change observation made on "observe()" on [OctoWorkspace].
pub struct OctoReaderForEvent<'update>(OctoReaderForEventInner<'update>, &'update OctoWorkspaceRef);
impl<'update> OctoRead for OctoReaderForEvent<'update> {
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

// mark non exhaustive so other crates must use the `new` constructor
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

impl From<(&str, &str)> for OctoBlockCreateOptions {
    fn from((block_id, flavor): (&str, &str)) -> Self {
        OctoBlockCreateOptions {
            id: block_id.into(),
            flavor: flavor.to_string(),
            properties: Default::default(),
        }
    }
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

/// Implements both [OctoRead] for reading refs out of and [OctoWrite] for mutating refs of an [OctoWorkspace].
///
/// Consider: Several places in the code use [OctoWriter] directly instead of `impl `[`OctoRead`].
/// So, there's a lack of symmetry & consistency with [OctoRead] as a result. Is that a problem?
pub struct OctoWriter<'doc> {
    doc_client_id: u64,
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn_mut: TransactionMut<'doc>,
}

impl<'doc> OctoRead for OctoWriter<'doc> {
    type YRSReadTxn = TransactionMut<'doc>;
    fn _parts(&self) -> _ReadParts<'_, Self::YRSReadTxn> {
        _ReadParts(&self.workspace_ref, &self.yrs_txn_mut)
    }
}

impl<'doc> OctoWrite<'doc> for OctoWriter<'doc> {
    fn _parts_mut(&mut self) -> _WriteParts<'_, TransactionMut<'doc>> {
        _WriteParts {
            operator: self.doc_client_id,
            workspace_ref: &self.workspace_ref,
            yrs_txn_mut: &mut self.yrs_txn_mut,
        }
    }
}

/// Implements [OctoRead] for reading refs out of an [OctoWorkspace].
pub struct OctoReader<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn: Transaction<'doc>,
}

impl<'doc> OctoRead for OctoReader<'doc> {
    type YRSReadTxn = Transaction<'doc>;

    fn _parts(&self) -> _ReadParts<'_, Transaction<'doc>> {
        _ReadParts(&self.workspace_ref, &self.yrs_txn)
    }
}

/// Create a private type like this so it's not possible for another crate to implement [OctoRead],
/// and prevent another crate from accessing underlying crate properties directly.
pub struct _ReadParts<'a, T>(&'a OctoWorkspaceRef, &'a T);

// Create a private type like this so it's not possible for another crate to implement [OctoWrite].
/// Only constructable from within the [crate].
pub struct _WriteParts<'a, T> {
    /// Client ID / Agent
    operator: u64,
    workspace_ref: &'a OctoWorkspaceRef,
    yrs_txn_mut: &'a mut T,
}

fn user_prop_key(unprefixed: &str) -> String {
    format!("prop:{unprefixed}")
}
fn remove_user_prop_prefix(prefixed: &str) -> Option<&str> {
    if prefixed.starts_with("prop:") {
        Some(prefixed.split_at(5).1)
    } else {
        None
    }
}

#[cfg(test)]
mod test_user_prop_keys {
    use super::{remove_user_prop_prefix, user_prop_key};
    #[track_caller]
    fn test(input: &str) {
        let as_prop = user_prop_key(input);
        assert_eq!(
            remove_user_prop_prefix(&as_prop),
            Some(input),
            "treated {input:?} as an unprefixed prop"
        );
    }

    #[test]
    fn remove_user_props() {
        assert_eq!(remove_user_prop_prefix("unprefixed"), None);
        assert_eq!(remove_user_prop_prefix(":prop:"), None);
        assert_eq!(remove_user_prop_prefix("sys:"), None);
        assert_eq!(remove_user_prop_prefix("prop:"), Some(""));
        assert_eq!(remove_user_prop_prefix("prop:abc"), Some("abc"));
    }

    #[test]
    fn add_user_props() {
        assert_eq!(user_prop_key("unprefixed"), "prop:unprefixed");
        assert_eq!(user_prop_key(":prop:"), "prop::prop:");
        assert_eq!(user_prop_key("sys:"), "prop:sys:");
        assert_eq!(user_prop_key("prop:"), "prop:prop:");
        assert_eq!(user_prop_key("prop:abc"), "prop:prop:abc");
    }

    #[test]
    fn adds_and_removes() {
        test("akjhwd");
        // can have colons?
        test("prop:");
        // can have colons?
        test(":::");
        // can be empty?
        test("");
    }
}

pub mod value {
    use lib0::any::Any;
    use yrs::types::Value;
    /// Value assigned to a block or metadata
    pub struct OctoValue(Value);

    impl OctoValue {
        pub fn try_as_string(&self) -> Result<String, OctoValueError> {
            match &self.0 {
                Value::Any(Any::String(box_str)) => return Ok(box_str.to_string()),
                _ => Err(OctoValueError::WrongType {
                    actual: self.actual_type(),
                    expected: "basic string",
                    details: None,
                }),
            }
        }

        // this is for constructing error only, not for stabilizing
        fn actual_type(&self) -> &'static str {
            match &self.0 {
                Value::Any(any_value) => match any_value {
                    Any::Null => "null",
                    Any::Undefined => "undefined",
                    Any::Bool(_) => "boolean",
                    Any::Number(_) => "number",
                    Any::BigInt(_) => "bigint",
                    Any::String(_) => "string",
                    Any::Buffer(_) => "buffer",
                    Any::Array(_) => "array",
                    Any::Map(_) => "map",
                },
                Value::YText(_) => "YText",
                Value::YArray(_) => "YArray",
                Value::YMap(_) => "YMap",
                Value::YXmlElement(_) => "YXmlElement",
                Value::YXmlFragment(_) => "YXmlFragment",
                Value::YXmlText(_) => "YXmlText",
                Value::YDoc(_) => "YDoc",
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum OctoValueError {
        // TODO: Can we reveal the underlying error in display (e.g. a parsing error?)
        #[error("expected value of type `{expected}`, but found `{actual}`")]
        WrongType {
            actual: &'static str,
            expected: &'static str,
            details: Option<Box<dyn std::error::Error>>,
        },
    }

    impl From<Value> for OctoValue {
        fn from(yrs_value: Value) -> Self {
            OctoValue(yrs_value)
        }
    }
}

pub trait OctoRead {
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
                "expected block({block_id:?}) to have both a (props map, and history array) or neither, but found {unexpected:?} respectively"
            ),
        }
    }

    fn get_workspace_metadata(&self, metadata_key: &str) -> Option<value::OctoValue> {
        let _ReadParts(workspace_ref, yrs_txn) = self._parts();
        let OctoWorkspaceRef {
            ref yrs_metadata_ref,
            ..
        } = workspace_ref;

        yrs_metadata_ref.get(yrs_txn, metadata_key).map(Into::into)
    }
}

pub trait OctoWrite<'doc>: OctoRead {
    fn _parts_mut(&mut self) -> _WriteParts<'_, TransactionMut<'doc>>;

    /// Create a block, returning an [OctoBlockRef] or erroring with [OctoCreateError].
    fn create_block<T: Into<OctoBlockCreateOptions>>(
        &mut self,
        options: T,
    ) -> Result<OctoBlockRef, errors::OctoCreateError> {
        let OctoBlockCreateOptions {
            id: new_block_id,
            properties,
            flavor,
        } = options.into();

        if self.contains_block(&new_block_id) {
            return Err(errors::OctoCreateError::IDConflict { id: new_block_id });
        }

        let _WriteParts {
            workspace_ref,
            yrs_txn_mut,
            operator,
        } = self._parts_mut();
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

        let block_ref = OctoBlockRef {
            id: new_block_id.into(),
            workspace_ref: workspace_ref.clone(),
            yrs_props_ref: props_ref,
            yrs_history_ref: history_ref,
        };

        // initial add operation
        block_ref.append_history(yrs_txn_mut, operator, HistoryOperation::Add);

        Ok(block_ref)
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

#[cfg(test)]
mod octo_playground_tests {
    use super::OctoBlockCreateOptions;
    use crate::{OctoWorkspace, OctoWrite};

    fn test_set_props<'doc, W: OctoWrite<'doc>>(writer: &mut W) {
        let block_1 = writer
            .create_block(OctoBlockCreateOptions {
                id: "abc".into(),
                flavor: "any-flavor-1".to_string(),
                properties: Default::default(),
            })
            .expect_display("created block");

        let block_flavor = block_1.try_flavor(&*writer).expect("has flavor");
        assert_eq!(block_flavor, "any-flavor-1");
    }

    // hmm
    trait ExpectDisplay<T> {
        fn expect_display(self, context: &str) -> T;
    }
    impl<T, E: std::error::Error> ExpectDisplay<T> for Result<T, E> {
        fn expect_display(self, context: &str) -> T {
            match self {
                Ok(val) => val,
                Err(err) => panic!("{context}: {err}"),
            }
        }
    }

    #[test]
    fn test_default_set_up() {
        let doc: yrs::Doc = Default::default();
        let mut ws = OctoWorkspace::from_doc(doc, "test_default_set_up-ws");
        let mut writer = ws.write();
        writer
            .create_block(OctoBlockCreateOptions {
                id: "abc".into(),
                flavor: "any-flavor-1".to_string(),
                properties: Default::default(),
            })
            .expect_display("created block");

        test_set_props(&mut writer);
    }

    #[test]
    fn test_screwed_up_blocks_errors() {
        let doc = {
            let doc: yrs::Doc = Default::default();
            let blocks_map = doc.get_or_insert_map("blocks");
            let updated_map = doc.get_or_insert_map("updated");

            doc
        };
        let mut ws = OctoWorkspace::from_doc(doc, "test_default_set_up-ws");
        // ws.yrs_doc_ref
        let mut write = ws.write();
        test_set_props(&mut write);
    }
}

impl OctoWorkspace {
    pub fn get_ref(&self) -> &OctoWorkspaceRef {
        &self.octo_ref
    }
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
    fn check_block_exists<R: yrs::ReadTxn>(
        &self,
        yrs_read_txn: &R,
    ) -> Result<(), errors::BlockNotFound> {
        // Is there a "fastest" way to determine if a MapRef is still a part of a document?

        // false for formatting
        if false
            || self
                .workspace_ref
                .yrs_block_ref
                .get(yrs_read_txn, &self.id)
                .and_then(|value| value.to_ymap())
                .is_none()
            || self
                .workspace_ref
                .yrs_updated_ref
                .get(yrs_read_txn, &self.id)
                .and_then(|value| value.to_yarray())
                .is_none()
        {
            return Err(errors::BlockNotFound {
                id: self.id.clone(),
            });
        }

        Ok(())
    }

    /// Internal direct access, helper
    fn try_get_yrs_prop_value_with_type_conversion<R, T, F>(
        &self,
        reader: &R,
        raw_key: &str,
        expected_type: &'static str,
        map_fn: F,
    ) -> Result<T, errors::OctoReadPropError>
    where
        R: OctoRead,
        // surfacing the original value ensures that we can print the incorrect value if there is a parsing failure.
        F: FnOnce(yrs::types::Value, &R::YRSReadTxn) -> Result<T, yrs::types::Value>,
    {
        self.try_get_yrs_prop_value(reader, raw_key, move |value, yrs_txn| {
            map_fn(value, yrs_txn).map_err(move |value| {
                // need value back to make "found" type string.
                errors::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    property_key: raw_key.into(),
                    expected_type,
                    found: value.to_string(yrs_txn),
                }
                .into()
            })
        })
    }
    /// Internal direct access
    fn try_get_yrs_prop_value<R, T, F>(
        &self,
        reader: &R,
        raw_key: &str,
        map_fn: F,
    ) -> Result<T, errors::OctoReadPropError>
    where
        R: OctoRead,
        F: FnOnce(yrs::types::Value, &R::YRSReadTxn) -> Result<T, errors::OctoReadPropError>,
    {
        let _ReadParts(workspace_ref, yrs_read_txn) = reader._parts();
        self.check_txn(workspace_ref)?;
        // If a block was deleted, should we still return its flavor if we have it?
        self.check_block_exists(yrs_read_txn)?;
        self.yrs_props_ref
            .get(yrs_read_txn, raw_key)
            .ok_or_else(|| {
                errors::BlockPropertyNotFound {
                    id: self.id.clone(),
                    property_key: raw_key.into(),
                }
                .into()
            })
            .and_then(|value| map_fn(value, yrs_read_txn))
    }

    fn append_history(
        &self,
        yrs_txn_mut: &mut TransactionMut,
        operator: u64,
        action: HistoryOperation,
    ) {
        use lib0::any::Any;
        let array = yrs::ArrayPrelim::from([
            Any::Number(operator as f64),
            Any::Number(chrono::Utc::now().timestamp_millis() as f64),
            Any::String(Box::from(action.to_string())),
        ]);

        self.yrs_history_ref.push_back(yrs_txn_mut, array);
    }

    fn try_set_yrs_prop_value<'doc, W: OctoWrite<'doc>>(
        &self,
        writer: &mut W,
        raw_key: &str,
        value: impl Into<lib0::any::Any>,
    ) -> Result<Option<yrs::types::Value>, errors::OctoUpdateError> {
        let _WriteParts {
            workspace_ref,
            yrs_txn_mut,
            operator,
        } = writer._parts_mut();
        self.check_txn(workspace_ref)?;
        self.check_block_exists(yrs_txn_mut)?;

        use lib0::any::Any;
        let (prev_value_opt, action) = match value.into() {
            Any::Bool(bool) => (
                self.yrs_props_ref.insert(yrs_txn_mut, raw_key, bool),
                HistoryOperation::Update,
            ),
            Any::String(text) => (
                self.yrs_props_ref
                    .insert(yrs_txn_mut, raw_key, text.to_string()),
                HistoryOperation::Update,
            ),
            Any::Number(number) => (
                self.yrs_props_ref.insert(yrs_txn_mut, raw_key, number),
                HistoryOperation::Update,
            ),
            Any::BigInt(number) => (
                if JS_INT_RANGE.contains(&number) {
                    self.yrs_props_ref
                        .insert(yrs_txn_mut, raw_key, number as f64)
                } else {
                    self.yrs_props_ref.insert(yrs_txn_mut, raw_key, number)
                },
                HistoryOperation::Update,
            ),
            Any::Null | Any::Undefined => (
                self.yrs_props_ref.remove(yrs_txn_mut, raw_key),
                HistoryOperation::Delete,
            ),
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {
                return Err(errors::OctoUpdateError::UnsupportedPropertyValue);
            }
        };

        // keep history up to date
        self.append_history(yrs_txn_mut, operator, action);

        Ok(prev_value_opt)
    }

    /// See [concepts::SysPropFlavor].
    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn flavor<'doc, R: OctoRead>(&self, reader: &R) -> String {
        self.try_flavor(reader).octo_unwrap()
    }

    /// See [concepts::SysPropFlavor].
    pub fn try_flavor<'doc, R: OctoRead>(
        &self,
        reader: &R,
    ) -> Result<String, errors::OctoReadPropError> {
        self.try_get_yrs_prop_value(reader, SYS_FLAVOR_KEY, |value, yrs_read_txn| {
            use yrs::types::Value;
            match value {
                Value::Any(lib0::any::Any::String(value)) => Ok(value.to_string()),
                other => Err(errors::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    property_key: SYS_FLAVOR_KEY.into(),
                    // TODO: be more consistent with these type names, yet.
                    expected_type: "Any::String",
                    found: other.to_string(yrs_read_txn),
                }
                .into()),
            }
        })
    }

    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn history(&self, reader: &impl OctoRead) -> Vec<BlockHistory> {
        self.try_history(reader).octo_unwrap()
    }

    /// See [concepts::YJSBlockHistory].
    pub fn try_history(
        &self,
        reader: &impl OctoRead,
    ) -> Result<Vec<BlockHistory>, errors::OctoReadHistoryError> {
        let _ReadParts(workspace_ref, yrs_read_txn) = reader._parts();
        self.check_txn(workspace_ref)?;
        self.check_block_exists(yrs_read_txn)?;

        self.yrs_history_ref
            .iter(yrs_read_txn)
            .map(|v| {
                let array = match v {
                    yrs::types::Value::YArray(array) => array,
                    other => {
                        return Err(crate::OctoAnyError::new(
                            format!(
                                "expected history item to be an array, but it was `{}`",
                                other.to_string(yrs_read_txn)
                            ),
                            None,
                        ))
                    }
                };

                // Note that from_yrs_array will use defaults if it fails to parse...
                BlockHistory::try_parse_yrs_array(array, yrs_read_txn, self.id.to_string())
            })
            .enumerate()
            .map(|(idx, result)| {
                result.map_err(|e| e.prefix_message(&format!("history item at index = {idx}")))
            })
            .collect::<Result<Vec<BlockHistory>, _>>()
            .map_err(errors::OctoReadHistoryError::BlockHistoryWasMalformed)
    }

    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn set_prop<'doc, W: OctoWrite<'doc>>(
        &self,
        writer: &mut W,
        unprefixed: &str,
        value: impl Into<lib0::any::Any>,
    ) -> Option<yrs::types::Value> {
        self.try_set_prop(writer, unprefixed, value).octo_unwrap()
    }

    /// Set a `value` under given user defined `key` into current map.
    /// Returns a value stored previously under the same key (if any existed).
    /// See [concepts::UserDefinedProp].
    pub fn try_set_prop<'doc, W: OctoWrite<'doc>>(
        &self,
        writer: &mut W,
        unprefixed: &str,
        value: impl Into<lib0::any::Any>,
    ) -> Result<Option<yrs::types::Value>, errors::OctoUpdateError> {
        debug_assert!(
            !unprefixed.contains(":"),
            "set_user_prop key should not contain colons, but found {unprefixed:?}"
        );

        self.try_set_yrs_prop_value(writer, &user_prop_key(unprefixed), value)
    }

    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn all_props(&self, reader: &impl OctoRead) -> HashMap<String, lib0::any::Any> {
        self.try_all_props(reader).octo_unwrap()
    }

    /// Set a `value` under given user defined `key` into current map.
    /// Returns a value stored previously under the same key (if any existed).
    /// See [concepts::UserDefinedProp].
    pub fn try_all_props(
        &self,
        reader: &impl OctoRead,
    ) -> Result<HashMap<String, lib0::any::Any>, errors::OctoReadPropError> {
        let _ReadParts(workspace_ref, yrs_txn) = reader._parts();
        self.check_txn(workspace_ref)?;

        let map = self.yrs_props_ref.iter(yrs_txn)
        .filter_map(|(prefixed_key, value)| {
            match (remove_user_prop_prefix(prefixed_key), value) {
                (Some(user_key), yrs::types::Value::Any(any_val)) => Some(Ok((user_key.to_string(), any_val))),
                (Some(user_key), other_type) => Some(Err(errors::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    // Should this be the prefixed key or unprefixed?
                    property_key: user_key.into(),
                    expected_type: "basic non Y type",
                    found: other_type.to_string(yrs_txn),
                })),
                _ => None,

            }
        })
        .collect::<Result<HashMap<String, lib0::any::Any>, errors::BlockPropertyWasUnexpectedType>>()?;

        Ok(map)
    }

    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn prop(&self, reader: &impl OctoRead, key: &str) -> Option<lib0::any::Any> {
        self.try_prop(reader, key).octo_unwrap()
    }

    /// Get a `value` under given user defined `key` into current map.
    /// Returns a value stored previously under the same key (if any existed).
    /// See [concepts::UserDefinedProp].
    pub fn try_prop(
        &self,
        reader: &impl OctoRead,
        key: &str,
    ) -> Result<Option<lib0::any::Any>, errors::OctoReadPropError> {
        let _ReadParts(workspace_ref, yrs_txn) = reader._parts();
        self.check_txn(workspace_ref)?;
        let prop_key = user_prop_key(key);
        let curr_val_opt = self.yrs_props_ref.get(yrs_txn, &prop_key);

        if let Some(curr_val) = curr_val_opt {
            return match curr_val {
                yrs::types::Value::Any(any_val) => Ok(Some(any_val)),
                _ => Err(errors::BlockPropertyWasUnexpectedType {
                    id: self.id.clone(),
                    property_key: prop_key.into(),
                    expected_type: "json",
                    found: "non-json type like YMap, YArray, YText".to_string(),
                }
                .into()),
            };
        }

        Ok(None)
    }

    pub fn id(&self) -> &str {
        self.id.as_ref()
    }

    #[track_caller]
    #[cfg(feature = "unwrap")]
    pub fn version(&self, reader: &impl OctoRead) -> [usize; 2] {
        self.try_version(reader).octo_unwrap()
    }

    /// block schema version
    /// for example: [1, 0]
    pub fn try_version(
        &self,
        reader: &impl OctoRead,
    ) -> Result<[usize; 2], errors::OctoReadPropError> {
        self.try_get_yrs_prop_value_with_type_conversion(
            reader,
            SYS_VERSION_KEY,
            "YArray with two positive integers, like [1, 0]",
            |value, yrs_txn| match value {
                yrs::types::Value::YArray(array) if array.len(yrs_txn) == 2 => {
                    let mut it = array
                        .iter(yrs_txn)
                        .map(|s| s.to_string(yrs_txn).parse::<usize>());
                    match (it.next(), it.next()) {
                        (Some(Ok(x0)), Some(Ok(x1))) => Ok([x0, x1]),
                        // surfacing the original value ensures that we can print the incorrect value if there is a parsing failure.
                        _ => Err(yrs::types::Value::YArray(array)),
                    }
                }
                // surfacing the original value ensures that we can print the incorrect value if there is a parsing failure.
                other => Err(other),
            },
        )
    }

    // pub fn created<T>(&self, trx: &T) -> u64
    // where
    //     T: ReadTxn,
    // {
    //     self.block
    //         .get(trx, "sys:created")
    //         .and_then(|c| match c.to_json(trx) {
    //             Any::Number(n) => Some(n as u64),
    //             _ => None,
    //         })
    //         .unwrap_or_default()
    // }
}

/// private convenience fn see [DifferentWorkspaces] error type.
trait AssociatedWorkspace {
    fn get_associated_workspace(&self) -> &OctoWorkspaceRef;
    fn check_txn<Other: AssociatedWorkspace>(
        &self,
        other: &Other,
    ) -> Result<(), errors::DifferentWorkspaces> {
        if self.get_associated_workspace().id != other.get_associated_workspace().id {
            Err(errors::DifferentWorkspaces(()))
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
pub mod errors {
    //! All errors that can result from directly interacting with [crate::OctoWorkspace] reading and writing.
    //!
    //! Internal:
    //! Methods on [crate::OctoWorkspace], [crate::OctoBlockRef], etc, should return `Octo*` enum error types.
    //! Other error types are designed to be single tuple content for each variant of the `Octo*` errors.
    //! For the methods which do very specific things like CRUD, keep errors very shallow to enhance match readability.
    //!
    //! Other parts of the library which work at a higher level and do many kinds of operations at a time might
    //! need more depth (wrapping `Octo*` errors in variants of higher level errors), but that should be avoided.
    use std::sync::Arc;

    use thiserror::Error;

    use crate::OctoAnyError;

    #[derive(Debug, Error)]
    pub enum OctoWorkspaceUpdateError {
        #[error("failed to parse binary of update")]
        FailedToParseBinary { source: Box<dyn std::error::Error> },
        #[error("failed to apply update")]
        FailedToApplyUpdate { panicked: Box<dyn std::any::Any> },
    }

    #[derive(Debug, Error)]
    pub enum OctoWorkspaceLoadError {
        #[error("failed to parse binary of workspace")]
        FailedToParseBinary { source: Box<dyn std::error::Error> },
    }

    /// Error type tag that gets mapped into other appropriate Octo errors like [OctoUpdateError::BlockNotFound]
    #[derive(Debug, Error)]
    #[error("block with id `{id:?}` not found")]
    pub struct BlockNotFound {
        pub id: Arc<str>,
    }

    /// Error type tag that gets mapped into other appropriate Octo errors like [OctoCreateError::TransactionFromDifferentWorkspace]
    #[derive(Error, Debug)]
    #[error("transaction was associated with a different workspace than the item")]
    pub struct DifferentWorkspaces(pub(crate) ());

    #[derive(Debug, Error)]
    pub enum OctoCreateError {
        /// Something with this ID already exists
        #[error("item with id `{id:?}` already exists")]
        IDConflict { id: Arc<str> },
        /// Tried to use a Transaction associated with a different Workspace
        #[error("transaction was associated with a different workspace than the item")]
        TransactionFromDifferentWorkspace(#[from] DifferentWorkspaces),
    }

    // from_diff_workspaces!(OctoCreateError);

    #[derive(Error, Debug)]
    pub enum OctoUpdateError {
        /// Tried to use a Transaction associated with a different Workspace
        #[error("transaction was associated with a different workspace than the item")]
        TransactionFromDifferentWorkspace(#[from] DifferentWorkspaces),

        #[error("property value provided was unsupported")]
        UnsupportedPropertyValue,

        /// Sometimes, you might have a block created from a different workspace
        #[error("block could not be found to be updated")]
        BlockNotFound(#[from] BlockNotFound),
    }

    #[derive(Error, Debug)]
    #[error("failed to read block property")]
    pub enum OctoReadPropError {
        #[error("failed to read block property; {0}")]
        TransactionFromDifferentWorkspace(#[from] DifferentWorkspaces),

        #[error("expected block property to have a different type; {0}")]
        BlockPropertyWasUnexpectedType(#[from] BlockPropertyWasUnexpectedType),

        #[error("failed to read block property; {0}")]
        BlockPropertyNotFound(#[from] BlockPropertyNotFound),

        #[error("failed to read block; {0}")]
        BlockNotFound(#[from] BlockNotFound),
    }

    #[derive(Error, Debug)]
    #[error("failed to read block history")]
    pub enum OctoReadHistoryError {
        #[error("failed to read history; {0}")]
        TransactionFromDifferentWorkspace(#[from] DifferentWorkspaces),

        // Simplistic error for now
        #[error("failed to read a malformed history; {0}")]
        BlockHistoryWasMalformed(OctoAnyError),

        #[error("failed to read history; {0}")]
        BlockNotFound(#[from] BlockNotFound),
    }

    #[derive(Debug, Error)]
    #[error("block `{id:?}` property `{property_key:?}` not found")]
    pub struct BlockPropertyNotFound {
        pub id: Arc<str>,
        pub property_key: Arc<str>,
    }

    #[derive(Debug, Error)]
    #[error("block with id `{id:?}` and property `{property_key}` expects `{expected_type}` but found `{found}`")]
    pub struct BlockPropertyWasUnexpectedType {
        pub id: Arc<str>,
        pub property_key: Arc<str>,
        pub expected_type: &'static str,
        pub found: String,
    }
}
