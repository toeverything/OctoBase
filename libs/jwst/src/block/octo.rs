use std::{collections::HashMap, rc::Rc};
use yrs::{
    types::ToJson, Array, ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, ReadTxn, Transaction,
};

#[derive(Debug, PartialEq)]
pub struct OctoWorkspaceRef {
    id: Rc<str>,
    // Perhaps we should include a way to
    // identify the operator / client id
    // via the Workspace's Awareness
}

/// A reference to a Block in a Workspace.
#[derive(Debug, PartialEq)]
pub struct OctoBlockRef {
    /// Unique Block ID
    id: Rc<str>,
    /// Retain workspace reference to improve troubleshooting if someone tries to use an [OctoBlockRef]
    /// from one [OctoWorkspaceRef] in a different [OctoWorkspaceRef].
    workspace_ref: OctoWorkspaceRef,
    /// Internal reference directly to the [yrs::MapRef] in the "Workspace"
    yrs_ref: yrs::MapRef,
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
    id: Rc<str>,
    /// Internal: Do not let anyone have this without requiring mutable access to [OctoWorkspace].
    doc: yrs::Doc,
    blocks: yrs::MapRef,

    /// For every change, we insert into this Map
    /// TODO: Evaluate the actual usefulness of this
    updated: yrs::MapRef,
}

pub struct OctoWorkspace {
    // /// What is this?
    // updates: yrs::MapRef,
}

struct OctoBlockCreateOptions {
    id: Rc<str>,
    flavor: String,
    // should properties also allow for other Prelims like Map/Array?
    properties: HashMap<String, lib0::any::Any>,
}

impl OctoBlockCreateOptions {
    fn new(id: Rc<str>, flavor: String, properties: HashMap<String, lib0::any::Any>) -> Self {
        Self {
            id,
            flavor,
            properties,
        }
    }
}

struct OctoTransactionMut<'doc> {
    /// Check to ensure that our Transaction matches the workspace we're operating on.
    workspace_ref: OctoWorkspaceRef,
    txn: TransactionMut<'doc>,
}

impl OctoWorkspace {
    pub fn get_ref(&self) -> OctoWorkspaceRef {
        OctoWorkspaceRef {
            id: self.id.clone(),
        }
    }

    fn check_txn(&self, txn: &OctoTransactionMut) -> Result<(), TxnFromDifferentWorkspace> {
        if self.id != txn.workspace_ref.id {
            Err(TxnFromDifferentWorkspace)
        } else {
            Ok(())
        }
    }

    /// Creating a block requires mutable access to ensure we can create a new item in the underlying store.
    pub fn create_block<T: Into<OctoBlockCreateOptions>>(
        &mut self,
        mut txn: &mut OctoTransactionMut,
        options: T,
    ) -> Result<OctoBlockRef, OctoCreateError> {
        let OctoBlockCreateOptions {
            id,
            properties,
            flavor,
        } = options.into();
        self.check_txn(&txn)?;

        if txn.txn.get_map(&id).is_some() {
            return Err(OctoCreateError::IDConflict { id });
        }

        // should not panic as we require exclusive borrow of self
        let block_ref = self.doc.get_or_insert_map(&id);

        // insert initial properties
        for (key, value) in properties {
            block_ref.insert(&mut txn.txn, key, value);
        }

        Ok(OctoBlockRef {
            id,
            workspace_ref: self.get_ref(),
            yrs_ref: block_ref,
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
        Ok(self.yrs_ref.insert(&mut txn.txn, key, value))
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
