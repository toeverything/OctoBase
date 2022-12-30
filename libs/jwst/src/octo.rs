//! Next iteration of the OctoBase design.
//! Progress 0/10:
//!
//! This is a clean rewrite of OctoBase concepts following the upgrade
//! of yrs 0.14 to support subdocs.
//!
//! At this moment, everything is in one file to create a pressure not to
//! over-complicate or over-expand the surface area of OctoBase core.
use lib0::any::Any;
use std::{collections::HashMap, rc::Rc};
use yrs::{ArrayPrelim, Map, MapPrelim, MapRef, TransactionMut};

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
    /// e.g.
    /// ```json
    /// "prop:text": "123"
    /// ```
    /// Or, in AFFiNE, it specifies it's own props like
    /// ```json
    /// "prop:xywh": "[0,0,720,480]",
    /// ```
    pub struct UserDefinedProp;

    /// Each time an edit or update happens to a Block, an event is inserted into the
    /// YJS Array this points to.
    ///
    /// Also see [YJSBlockPropMap].
    pub struct YJSBlockHistory;
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

pub struct OctoWorkspaceContent {
    /// Internal: Do not let anyone have this without requiring mutable access to [OctoWorkspaceContent].
    doc: yrs::Doc,
    octo_ref: OctoWorkspaceRef,
}

pub struct OctoWorkspace {
    /// Separated data type to be surfaced to workspace plugins.
    content: OctoWorkspaceContent,
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

pub struct OctoTransactionMut<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    yrs_txn_mut: TransactionMut<'doc>,
}

impl OctoWorkspace {
    pub fn get_ref(&self) -> &OctoWorkspaceRef {
        &self.content.octo_ref
    }

    fn check_txn(&self, txn: &OctoTransactionMut) -> Result<(), TxnFromDifferentWorkspace> {
        if self.content.octo_ref != txn.workspace_ref {
            Err(TxnFromDifferentWorkspace)
        } else {
            Ok(())
        }
    }

    /// Creating a block requires mutable access to ensure we can create a new item in the underlying store.
    pub fn create_block<T: Into<OctoBlockCreateOptions>>(
        &self,
        mut txn: &mut OctoTransactionMut,
        options: T,
    ) -> Result<OctoBlockRef, OctoCreateError> {
        let OctoBlockCreateOptions {
            id: new_block_id,
            properties,
            flavor,
        } = options.into();
        self.check_txn(&txn)?;
        let ws_ref = &self.content.octo_ref;

        let (props_ref, history_ref) = {
            if ws_ref
                .yrs_block_ref
                .contains(&txn.yrs_txn_mut, &new_block_id)
                || ws_ref
                    .yrs_updated_ref
                    .contains(&txn.yrs_txn_mut, &new_block_id)
            {
                return Err(OctoCreateError::IDConflict { id: new_block_id });
            }

            // should not panic because we required exclusive borrow of self
            let preexisting_values = (
                ws_ref.yrs_block_ref.insert(
                    &mut txn.yrs_txn_mut,
                    new_block_id.clone(),
                    MapPrelim::<Any>::new(),
                ),
                ws_ref.yrs_updated_ref.insert(
                    &mut txn.yrs_txn_mut,
                    new_block_id.clone(),
                    ArrayPrelim::<Vec<String>, String>::from(vec![]),
                ),
            );

            #[cfg(debug_assertions)]
            if preexisting_values.0.is_some() || preexisting_values.1.is_some() {
                unreachable!("Should have been an ID Conflict for {new_block_id:?}. Found values already existing for block: {preexisting_values:?}");
            }

            (
                ws_ref
                    .yrs_block_ref
                    .get(&txn.yrs_txn_mut, &new_block_id)
                    .and_then(|b| b.to_ymap())
                    .expect("new block props is map"),
                ws_ref
                    .yrs_updated_ref
                    .get(&txn.yrs_txn_mut, &new_block_id)
                    .and_then(|b| b.to_yarray())
                    .expect("new block updated is array"),
            )
        };

        // See https://doc.rust-lang.org/rust-by-example/macros/repeat.html
        macro_rules! insert_prop_and_prelim_pairs {
            ($(($id:expr, $val:expr)),+) => { $(props_ref.insert(&mut txn.yrs_txn_mut, $id, $val);)+ }
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
            props_ref.insert(&mut txn.yrs_txn_mut, format!("prop:{key}"), value);
        }

        Ok(OctoBlockRef {
            id: new_block_id,
            workspace_ref: self.get_ref().clone(),
            yrs_props_ref: props_ref,
            yrs_history_ref: history_ref,
        })
    }
}

impl OctoBlockRef {
    fn check_txn(&self, txn: &OctoTransactionMut) -> Result<(), TxnFromDifferentWorkspace> {
        if self.workspace_ref.id != txn.workspace_ref.id {
            Err(TxnFromDifferentWorkspace)
        } else {
            Ok(())
        }
    }

    /// Inserts a new `value` under given `key` into current map. Returns a value stored previously
    /// under the same key (if any existed).
    pub fn insert(
        &self,
        mut txn: &mut OctoTransactionMut,
        key: &str,
        value: lib0::any::Any,
    ) -> Result<Option<yrs::types::Value>, OctoCreateError> {
        self.check_txn(&txn)?;
        Ok(self.yrs_props_ref.insert(&mut txn.yrs_txn_mut, key, value))
    }

    pub fn id(&self) -> &str {
        self.id.as_ref()
    }
}

#[derive(Debug)]
pub enum OctoError {
    /// Sometimes, you might have a block created from a different workspace
    BlockNotFound {
        id: Rc<str>,
        detail: OctoErrorBlockNotFoundDetail,
    },
    /// Tried to use a Transaction associated with a different Workspace
    TransactionFromDifferentWorkspace,
}

struct TxnFromDifferentWorkspace;

impl From<TxnFromDifferentWorkspace> for OctoError {
    fn from(_: TxnFromDifferentWorkspace) -> Self {
        Self::TransactionFromDifferentWorkspace
    }
}

#[derive(Debug)]
pub enum OctoCreateError {
    /// Something with this ID already exists
    IDConflict { id: Rc<str> },
    /// Tried to use a Transaction associated with a different Workspace
    TransactionFromDifferentWorkspace,
}

impl From<TxnFromDifferentWorkspace> for OctoCreateError {
    fn from(_: TxnFromDifferentWorkspace) -> Self {
        Self::TransactionFromDifferentWorkspace
    }
}

#[derive(Debug)]
pub enum OctoErrorBlockNotFoundDetail {
    /// The block is from a different workspace
    BlockBelongsToDifferentWorkspace,
    /// The block was created in a different version of the workspace
    BlockFromAFutureVersionOfTheWorkspace,
}
